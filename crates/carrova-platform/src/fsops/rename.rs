//! Renames that never follow the final component.

use std::io;
use std::path::Path;

use rustix::fs::{CWD, RenameFlags};

use super::errno;
use crate::PlatformError;

/// See [`crate::Platform::rename_noreplace`].
pub(crate) fn rename_noreplace(from: &Path, to: &Path) -> Result<(), PlatformError> {
    rustix::fs::renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(|e| {
        let source = io::Error::from(e);
        // Report the destination when it is the cause; otherwise the source.
        let at = if source.kind() == io::ErrorKind::AlreadyExists {
            to
        } else {
            from
        };
        PlatformError::io(at, source)
    })
}

/// See [`crate::Platform::rename_replace`].
pub(crate) fn rename_replace(from: &Path, to: &Path) -> Result<(), PlatformError> {
    rustix::fs::renameat(CWD, from, CWD, to).map_err(errno(from))
}
