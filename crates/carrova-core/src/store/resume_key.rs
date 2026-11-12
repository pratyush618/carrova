//! The LAN resume secret of a session (`resume.key`): written once, read
//! only if nobody else could have read or planted it, wiped when the
//! session ends. File handling lives in `secret_file`.

use std::path::Path;

use carrova_crypto::pairing::{RESUME_SECRET_LEN, ResumeSecret};
use carrova_platform::Platform;

use super::RESUME_KEY_FILE;
use super::secret_file;
use crate::error::{CoreError, Result};

/// Store the LAN resume secret of a session as `session_dir/resume.key`:
/// created new (`O_EXCL|O_NOFOLLOW`, never through a link or over an
/// existing file), mode `600`, synced. Deleted with
/// [`wipe_resume_secret`] when the session ends.
pub fn write_resume_secret(
    platform: &dyn Platform,
    session_dir: &Path,
    secret: &ResumeSecret,
) -> Result<()> {
    secret_file::write_new(
        platform,
        session_dir,
        RESUME_KEY_FILE,
        secret.expose_bytes(),
    )
}

/// The session's LAN resume secret, or `None` if it has none (never
/// paired, or ended). Refused ([`CoreError::ResumeKey`]) unless the opened
/// file is a regular file of mode `600` owned by the current user and
/// exactly the secret's size (ACL platforms excepted): a key others could
/// read or plant is never used.
pub fn read_resume_secret(
    platform: &dyn Platform,
    session_dir: &Path,
) -> Result<Option<ResumeSecret>> {
    let bytes = secret_file::read::<RESUME_SECRET_LEN>(
        platform,
        &session_dir.join(RESUME_KEY_FILE),
        CoreError::ResumeKey,
    )?;
    Ok(bytes.map(|b| ResumeSecret::from_bytes(*b)))
}

/// Destroy the session's resume secret: overwrite it with zeros, sync,
/// unlink, sync the dir. Missing is fine. Runs automatically when a
/// session record reaches a terminal state ([`super::Store::save`]).
pub fn wipe_resume_secret(platform: &dyn Platform, session_dir: &Path) -> Result<()> {
    secret_file::wipe(platform, session_dir, RESUME_KEY_FILE, RESUME_SECRET_LEN)
}
