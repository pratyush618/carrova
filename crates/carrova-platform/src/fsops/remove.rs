//! Removing files and confined directory trees via fds.

use std::ffi::{CStr, OsStr};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fs::{AtFlags, FileType, Mode, OFlags};

use super::dir::open_dir_nofollow;
use super::{errno, invalid};
use crate::PlatformError;
use crate::path::normalize_abs;

/// See [`crate::Platform::remove_file`].
pub(crate) fn remove_file(path: &Path) -> Result<(), PlatformError> {
    // unlink(2) never follows the final component and refuses directories.
    rustix::fs::unlink(path).map_err(errno(path))
}

/// See [`crate::Platform::remove_dir_all_confined`].
///
/// `within` itself is opened normally (the caller vouches for it); every
/// component below it is opened `O_NOFOLLOW|O_DIRECTORY` relative to its
/// parent fd, so a symlink anywhere between `within` and the deleted entries
/// aborts the walk instead of redirecting it. A missing `path` is a
/// `NotFound` error.
pub(crate) fn remove_dir_all_confined(path: &Path, within: &Path) -> Result<(), PlatformError> {
    let not_inside = || {
        invalid(
            path,
            "path is not strictly inside the confinement directory",
        )
    };
    let norm_within = normalize_abs(within).ok_or_else(|| invalid(within, "not absolute"))?;
    let norm_path = normalize_abs(path).ok_or_else(not_inside)?;
    let rel = norm_path
        .strip_prefix(&norm_within)
        .map_err(|_| not_inside())?;
    let mut comps: Vec<&OsStr> = rel.iter().collect();
    let Some(last) = comps.pop() else {
        return Err(not_inside());
    };

    let mut dir = rustix::fs::open(
        &norm_within,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(errno(within))?;
    for c in comps {
        dir = open_dir_nofollow(&dir, c, path)?;
    }
    remove_tree_at(&dir, last, path)
}

/// Delete entry `name` of directory `parent`, recursing into directories
/// via fds (never via paths), unlinking symlinks without traversal.
fn remove_tree_at(parent: &OwnedFd, name: &OsStr, report: &Path) -> Result<(), PlatformError> {
    let st = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(errno(report))?;
    if FileType::from_raw_mode(st.st_mode) != FileType::Directory {
        return rustix::fs::unlinkat(parent, name, AtFlags::empty()).map_err(errno(report));
    }
    // If `name` was swapped for a symlink since statat, this open fails.
    let dir = open_dir_nofollow(parent, name, report)?;
    let mut children = Vec::new();
    for entry in rustix::fs::Dir::read_from(&dir).map_err(errno(report))? {
        let entry = entry.map_err(errno(report))?;
        let child: &CStr = entry.file_name();
        let bytes = child.to_bytes();
        if bytes != b"." && bytes != b".." {
            children.push(OsStr::from_bytes(bytes).to_owned());
        }
    }
    for child in &children {
        remove_tree_at(&dir, child, report)?;
    }
    rustix::fs::unlinkat(parent, name, AtFlags::REMOVEDIR).map_err(errno(report))
}
