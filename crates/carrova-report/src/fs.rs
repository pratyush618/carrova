//! The file operations the store needs from the caller's platform layer.
//!
//! Reports and the consent choice must be private to the user (Unix
//! `0600`), which needs OS-specific calls this OS-neutral crate does not
//! make. Front-ends implement [`ReportFs`] over their `Platform`.

use std::fmt;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use crate::error::ReportError;

/// Owner-only file creation and atomic replacement.
pub trait ReportFs: fmt::Debug + Send + Sync {
    /// Create a new regular file at `path`, readable and writable by its
    /// owner only (Unix `0600`, independent of umask). Fails if anything
    /// exists at `path`; never follows a symlink there.
    ///
    /// # Errors
    ///
    /// The file cannot be created.
    fn create_private(&self, path: &Path) -> io::Result<File>;

    /// Atomically rename `from` to `to`, replacing `to` if it exists.
    ///
    /// # Errors
    ///
    /// The rename fails.
    fn replace(&self, from: &Path, to: &Path) -> io::Result<()>;
}

/// Write `bytes` to `path` atomically as a private file: into the fresh
/// temporary `tmp` (a leftover from an interrupted write is removed
/// first), flushed, then moved over `path`.
pub(crate) fn write_private(
    fs: &dyn ReportFs,
    tmp: &Path,
    path: &Path,
    bytes: &[u8],
) -> Result<(), ReportError> {
    match std::fs::remove_file(tmp) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(ReportError::io(tmp)(err)),
        _ => {}
    }
    let mut file = fs.create_private(tmp).map_err(ReportError::io(tmp))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(ReportError::io(tmp))?;
    fs.replace(tmp, path).map_err(ReportError::io(path))
}
