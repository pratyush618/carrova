//! Opening existing regular files (and reading links) without following
//! symlinks.

use std::fs::File;
use std::path::{Path, PathBuf};

use rustix::fs::{FileType, Mode, OFlags};

use super::{errno, file_type, invalid};
use crate::PlatformError;

/// See [`crate::Platform::open_read_nofollow`].
pub(crate) fn open_read_nofollow(path: &Path) -> Result<File, PlatformError> {
    // NONBLOCK: opening a FIFO must not hang; it is rejected right after.
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NOCTTY;
    let fd =
        rustix::fs::open(path, flags | OFlags::NONBLOCK, Mode::empty()).map_err(errno(path))?;
    if file_type(path, &fd)? != FileType::RegularFile {
        return Err(invalid(path, "not a regular file"));
    }
    // Back to blocking mode for ordinary reads.
    let fl = rustix::fs::fcntl_getfl(&fd).map_err(errno(path))?;
    rustix::fs::fcntl_setfl(&fd, fl - OFlags::NONBLOCK).map_err(errno(path))?;
    Ok(File::from(fd))
}

/// See [`crate::Platform::open_append_nofollow`].
pub(crate) fn open_append_nofollow(path: &Path) -> Result<File, PlatformError> {
    // NONBLOCK: a FIFO must not hang the open; it is rejected right after.
    let flags =
        OFlags::WRONLY | OFlags::APPEND | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NOCTTY;
    let fd =
        rustix::fs::open(path, flags | OFlags::NONBLOCK, Mode::empty()).map_err(errno(path))?;
    if file_type(path, &fd)? != FileType::RegularFile {
        return Err(invalid(path, "not a regular file"));
    }
    let fl = rustix::fs::fcntl_getfl(&fd).map_err(errno(path))?;
    rustix::fs::fcntl_setfl(&fd, fl - OFlags::NONBLOCK).map_err(errno(path))?;
    Ok(File::from(fd))
}

/// See [`crate::Platform::open_rw_nofollow`].
pub(crate) fn open_rw_nofollow(path: &Path) -> Result<File, PlatformError> {
    // NONBLOCK: a FIFO must not hang the open; it is rejected right after.
    let flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NOCTTY;
    let fd =
        rustix::fs::open(path, flags | OFlags::NONBLOCK, Mode::empty()).map_err(errno(path))?;
    if file_type(path, &fd)? != FileType::RegularFile {
        return Err(invalid(path, "not a regular file"));
    }
    let fl = rustix::fs::fcntl_getfl(&fd).map_err(errno(path))?;
    rustix::fs::fcntl_setfl(&fd, fl - OFlags::NONBLOCK).map_err(errno(path))?;
    Ok(File::from(fd))
}

/// See [`crate::Platform::read_link`].
pub(crate) fn read_link(path: &Path) -> Result<PathBuf, PlatformError> {
    std::fs::read_link(path).map_err(|e| PlatformError::io(path, e))
}
