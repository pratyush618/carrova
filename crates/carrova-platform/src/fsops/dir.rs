//! Listing directories and opening them without following symlinks.

use std::ffi::OsStr;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fs::{AtFlags, CWD, FileType, Mode, OFlags};

use super::errno;
use crate::{DirEntry, FileKind, PlatformError};

/// See [`crate::Platform::read_dir`].
pub(crate) fn read_dir(path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
    let dir = open_dir_nofollow(CWD, path, path)?;
    let mut entries = Vec::new();
    for entry in rustix::fs::Dir::read_from(&dir).map_err(errno(path))? {
        let entry = entry.map_err(errno(path))?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let name = OsStr::from_bytes(bytes);
        let file_type = match entry.file_type() {
            // Some filesystems leave `d_type` unset: ask without following.
            FileType::Unknown => {
                let st = rustix::fs::statat(&dir, name, AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(errno(path))?;
                FileType::from_raw_mode(st.st_mode)
            }
            known => known,
        };
        entries.push(DirEntry {
            name: name.to_owned(),
            kind: file_kind(file_type),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn file_kind(file_type: FileType) -> FileKind {
    match file_type {
        FileType::RegularFile => FileKind::File,
        FileType::Directory => FileKind::Dir,
        FileType::Symlink => FileKind::Symlink,
        _ => FileKind::Other,
    }
}

pub(super) fn open_dir_nofollow(
    parent: impl AsFd,
    name: impl rustix::path::Arg,
    report: &Path,
) -> Result<OwnedFd, PlatformError> {
    rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(errno(report))
}
