//! Secret key files (a session's resume key, the device key, known-device
//! keys): created without following links, mode `600`, read only if
//! nobody else could have read or planted them, zeroed before unlinking.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use carrova_platform::{FileKind, PermissionModel, Platform};
use zeroize::Zeroizing;

use super::files::open_existing;
use crate::error::{CoreError, KeyFileProblem, Result};
use crate::fsx;

/// Mode a secret file must have.
const SECRET_MODE: u32 = 0o600;

/// Store `bytes` as the new file `dir/name`: created new
/// (`O_EXCL|O_NOFOLLOW`, never through a link or over an existing file),
/// mode `600`, synced, then the dir synced. The file is wiped on any
/// failure (unless create-new failed because something already exists).
pub(crate) fn write_new(
    platform: &dyn Platform,
    dir: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    let mut f = platform.create_new_file(&dir.join(name), Some(SECRET_MODE))?;
    // `move`: the file is closed before any wipe (Windows cannot remove an
    // open file).
    let written = (move || -> Result<()> {
        f.write_all(bytes)?;
        f.sync_all()?;
        platform.sync_dir(dir)?;
        Ok(())
    })();
    if written.is_err() {
        wipe_after_failure(platform, dir, name, bytes.len());
    }
    written
}

/// Replace `dir/name` with `bytes`: written as `name.new` like
/// [`write_new`] (a stale one from a crash is wiped first), renamed over
/// the old file, dir synced. The old file is opened before the rename and
/// zeroed after it, so its secret does not stay on disk and a crash never
/// leaves a zeroed file in place of a key.
pub(crate) fn replace(platform: &dyn Platform, dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let new = format!("{name}.new");
    wipe(platform, dir, &new, bytes.len())?;
    write_new(platform, dir, &new, bytes)?;
    // An old file that cannot be opened (read-only) is still replaced,
    // like one that cannot be zeroed: refusing would leave a bad key stuck.
    let old = open_regular(platform, &dir.join(name)).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "could not open a key file to zero it");
        None
    });
    let renamed = platform
        .rename_replace(&dir.join(&new), &dir.join(name))
        .map_err(CoreError::from)
        .and_then(|()| platform.sync_dir(dir).map_err(Into::into));
    if renamed.is_err() {
        wipe_after_failure(platform, dir, &new, bytes.len());
        return renamed;
    }
    if let Some((mut file, size)) = old
        && let Err(e) = zero(&mut file, size, bytes.len())
    {
        tracing::warn!(error = %e, "could not zero a replaced key file");
    }
    Ok(())
}

/// The regular file at `path` opened read-write without following links,
/// with its size; `None` if there is none (or something else is there).
fn open_regular(platform: &dyn Platform, path: &Path) -> Result<Option<(std::fs::File, u64)>> {
    match fsx::lstat(platform, path)? {
        Some(meta) if meta.kind == FileKind::File => {
            Ok(Some((platform.open_rw_nofollow(path)?, meta.size)))
        }
        _ => Ok(None),
    }
}

/// Overwrite the first `len` bytes (at most `size`) of `file` with zeros
/// and sync.
fn zero(file: &mut std::fs::File, size: u64, len: usize) -> Result<()> {
    let len = usize::try_from(size).map_or(len, |n| n.min(len));
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&vec![0u8; len])?;
    file.sync_all()?;
    Ok(())
}

/// The `N`-byte secret at `path`, or `None` if there is none. Refused
/// (`refuse(problem)`) unless the opened file (fstat, so a path swapped
/// after the open changes nothing) is a regular file of mode `600` owned
/// by the current user (when the platform knows users) and exactly `N`
/// bytes. A platform that reports no permission bits is refused too,
/// except under [`PermissionModel::Acl`]: there the profile's ACLs keep
/// the state directory private, and neither mode nor owner is checked.
pub(crate) fn read<const N: usize>(
    platform: &dyn Platform,
    path: &Path,
    refuse: impl Fn(KeyFileProblem) -> CoreError,
) -> Result<Option<Zeroizing<[u8; N]>>> {
    let Some(file) = open_existing(platform, path)? else {
        return Ok(None);
    };
    if let Some(problem) = check_meta(platform, &file)? {
        return Err(refuse(problem));
    }
    let mut buf = Zeroizing::new(Vec::with_capacity(N + 1));
    file.take(N as u64 + 1).read_to_end(&mut buf)?;
    if buf.len() != N {
        return Err(refuse(KeyFileProblem::WrongSize));
    }
    let mut secret = Zeroizing::new([0u8; N]);
    secret.copy_from_slice(&buf);
    Ok(Some(secret))
}

/// [`read`]'s permission and owner check of the opened file (opened
/// without following links).
fn check_meta(platform: &dyn Platform, file: &std::fs::File) -> Result<Option<KeyFileProblem>> {
    if platform.permission_model() == PermissionModel::Acl {
        return Ok(None);
    }
    let meta = platform.file_metadata(file)?;
    Ok(match meta.mode {
        None => Some(KeyFileProblem::UnknownMode),
        Some(mode) if mode & 0o7777 != SECRET_MODE => Some(KeyFileProblem::Mode {
            found: mode,
            required: SECRET_MODE,
        }),
        Some(_) => match (meta.uid, platform.current_uid()) {
            (Some(owner), Some(me)) if owner != me => Some(KeyFileProblem::Owner { owner, me }),
            _ => None,
        },
    })
}

/// Destroy the secret `dir/name` (at most `len` bytes long when valid):
/// overwrite it with zeros, sync, unlink, sync the dir. Missing is fine;
/// a planted link is unlinked, never followed.
pub(crate) fn wipe(platform: &dyn Platform, dir: &Path, name: &str, len: usize) -> Result<()> {
    let path = dir.join(name);
    if fsx::lstat(platform, &path)?.is_none() {
        return Ok(());
    }
    if let Some((mut file, size)) = open_regular(platform, &path)? {
        zero(&mut file, size, len)?;
    }
    platform.remove_file(&path)?;
    platform.sync_dir(dir)?;
    Ok(())
}

/// Best-effort [`wipe`] of a partial file after a failed write.
fn wipe_after_failure(platform: &dyn Platform, dir: &Path, name: &str, len: usize) {
    if let Err(e) = wipe(platform, dir, name, len) {
        tracing::warn!(error = %e, "could not remove a partial key file");
    }
}
