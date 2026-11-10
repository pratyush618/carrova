//! Shared lstat / fstat implementation used by native adapters and the
//! fake platform.

use std::fs::{File, Metadata};
use std::io;
use std::path::Path;

use crate::{FileKind, FileMeta, PlatformError};

/// Read metadata without following symlinks.
pub(crate) fn lstat(path: &Path) -> Result<FileMeta, PlatformError> {
    let md = std::fs::symlink_metadata(path).map_err(|e| PlatformError::io(path, e))?;
    Ok(file_meta(&md))
}

/// Metadata of an open file (fstat): describes exactly the opened inode,
/// whatever its path points to now.
pub(crate) fn fstat(file: &File) -> io::Result<FileMeta> {
    file.metadata().map(|md| file_meta(&md))
}

fn file_meta(md: &Metadata) -> FileMeta {
    let ft = md.file_type();
    let kind = if ft.is_symlink() {
        FileKind::Symlink
    } else if ft.is_dir() {
        FileKind::Dir
    } else if ft.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    };

    #[cfg(unix)]
    let (mode, uid, gid) = {
        use std::os::unix::fs::MetadataExt;
        (Some(md.mode() & 0o7777), Some(md.uid()), Some(md.gid()))
    };
    #[cfg(not(unix))]
    let (mode, uid, gid) = (None, None, None);

    FileMeta {
        kind,
        size: md.len(),
        mode,
        uid,
        gid,
        modified: md.modified().ok(),
    }
}
