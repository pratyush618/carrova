//! The LAN resume secret of a session (`resume.key`): written once, read
//! only if nobody else could have read or planted it, wiped when the
//! session ends.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use carrova_crypto::pairing::{RESUME_SECRET_LEN, ResumeSecret};
use carrova_platform::{FileKind, Platform};

use super::RESUME_KEY_FILE;
use super::files::open_existing;
use crate::error::{CoreError, Result, ResumeKeyProblem};
use crate::fsx;

/// Mode a resume key must have.
const RESUME_KEY_MODE: u32 = 0o600;

/// Store the LAN resume secret of a session as `session_dir/resume.key`:
/// created new (`O_EXCL|O_NOFOLLOW`, never through a link or over an
/// existing file), mode `600`, synced. Deleted with
/// [`wipe_resume_secret`] when the session ends.
pub fn write_resume_secret(
    platform: &dyn Platform,
    session_dir: &Path,
    secret: &ResumeSecret,
) -> Result<()> {
    let path = session_dir.join(RESUME_KEY_FILE);
    let result = (|| -> Result<()> {
        let mut f = platform.create_new_file(&path, Some(RESUME_KEY_MODE))?;
        f.write_all(secret.expose_bytes())?;
        f.sync_all()?;
        Ok(())
    })();
    match result {
        Ok(()) => platform.sync_dir(session_dir).map_err(Into::into),
        Err(e) => {
            // Never leave a partial key behind (unless the file was not
            // ours: create-new failed because something already exists).
            if fsx::kind_of(&e) != Some(std::io::ErrorKind::AlreadyExists)
                && let Err(w) = wipe_resume_secret(platform, session_dir)
            {
                tracing::warn!(error = %w, "could not remove partial resume key");
            }
            Err(e)
        }
    }
}

/// The session's LAN resume secret, or `None` if it has none (never
/// paired, or ended). Refused ([`CoreError::ResumeKey`]) unless the opened
/// file (fstat, so a path swapped after the open changes nothing) is a
/// regular file of mode `600` owned by the current user (when the platform
/// knows users) and exactly the secret's size: a key others could read or
/// plant is never used. A platform that reports no permission bits is
/// refused too.
pub fn read_resume_secret(
    platform: &dyn Platform,
    session_dir: &Path,
) -> Result<Option<ResumeSecret>> {
    let Some(file) = open_existing(platform, &session_dir.join(RESUME_KEY_FILE))? else {
        return Ok(None);
    };
    check_key_meta(platform, &file)?;
    let mut buf = zeroize::Zeroizing::new(Vec::with_capacity(RESUME_SECRET_LEN + 1));
    file.take(RESUME_SECRET_LEN as u64 + 1)
        .read_to_end(&mut buf)?;
    if buf.len() != RESUME_SECRET_LEN {
        return Err(CoreError::ResumeKey(ResumeKeyProblem::WrongSize));
    }
    let mut bytes = [0u8; RESUME_SECRET_LEN];
    bytes.copy_from_slice(&buf);
    let secret = ResumeSecret::from_bytes(bytes);
    zeroize::Zeroize::zeroize(&mut bytes);
    Ok(Some(secret))
}

/// [`read_resume_secret`]'s permission and owner check of the opened key
/// file (opened without following links).
fn check_key_meta(platform: &dyn Platform, file: &std::fs::File) -> Result<()> {
    let meta = platform.file_metadata(file)?;
    let problem = match meta.mode {
        None => Some(ResumeKeyProblem::UnknownMode),
        Some(mode) if mode & 0o7777 != RESUME_KEY_MODE => Some(ResumeKeyProblem::Mode {
            found: mode,
            required: RESUME_KEY_MODE,
        }),
        Some(_) => match (meta.uid, platform.current_uid()) {
            (Some(owner), Some(me)) if owner != me => Some(ResumeKeyProblem::Owner { owner, me }),
            _ => None,
        },
    };
    problem.map_or(Ok(()), |p| Err(CoreError::ResumeKey(p)))
}

/// Destroy the session's resume secret: overwrite it with zeros, sync,
/// unlink, sync the dir. Missing is fine. Runs automatically when a
/// session record reaches a terminal state ([`super::Store::save`]).
pub fn wipe_resume_secret(platform: &dyn Platform, session_dir: &Path) -> Result<()> {
    let path = session_dir.join(RESUME_KEY_FILE);
    let Some(meta) = fsx::lstat(platform, &path)? else {
        return Ok(());
    };
    if meta.kind == FileKind::File {
        let mut f = platform.open_rw_nofollow(&path)?;
        // Only the first RESUME_SECRET_LEN bytes can hold a valid secret.
        let len =
            usize::try_from(meta.size).map_or(RESUME_SECRET_LEN, |n| n.min(RESUME_SECRET_LEN));
        f.seek(SeekFrom::Start(0))?;
        f.write_all(&[0u8; RESUME_SECRET_LEN][..len])?;
        f.sync_all()?;
    }
    // Not a regular file (a planted link): just unlink it, never follow.
    platform.remove_file(&path)?;
    platform.sync_dir(session_dir)?;
    Ok(())
}
