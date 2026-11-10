//! Durability and positional I/O: filesystem ids, syncs, preallocation.

use std::fs::File;
use std::io;
use std::path::Path;

use rustix::fs::{CWD, FileType, Mode, OFlags};

use super::dir::open_dir_nofollow;
use super::{errno, file_type, invalid};
use crate::{Allocation, FsId, PlatformError};

/// See [`crate::Platform::fs_id`].
pub(crate) fn fs_id(path: &Path) -> Result<FsId, PlatformError> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).map_err(|e| PlatformError::io(path, e))?;
    Ok(FsId::new(meta.dev()))
}

/// See [`crate::Platform::sync_fs`]: `syncfs(2)` on an `O_NOFOLLOW` fd of
/// `path` (a directory or regular file).
pub(crate) fn sync_fs(path: &Path) -> Result<(), PlatformError> {
    let fd = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(errno(path))?;
    match file_type(path, &fd)? {
        FileType::RegularFile | FileType::Directory => {}
        _ => return Err(invalid(path, "not a regular file or directory")),
    }
    rustix::fs::syncfs(&fd).map_err(errno(path))
}

/// See [`crate::Platform::write_all_at`].
pub(crate) fn write_all_at(file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(file, buf, offset)
}

/// See [`crate::Platform::preallocate`]: `fallocate(2)` (mode 0) reserves
/// the blocks, so a full disk fails here (`StorageFull`) instead of
/// mid-write. Filesystems without `fallocate` get a sparse `ftruncate`.
pub(crate) fn preallocate(file: &File, len: u64) -> io::Result<Allocation> {
    if len == 0 {
        file.set_len(0)?;
        return Ok(Allocation::Reserved);
    }
    match rustix::fs::fallocate(file, rustix::fs::FallocateFlags::empty(), 0, len) {
        Ok(()) => Ok(Allocation::Reserved),
        Err(e) if e == rustix::io::Errno::OPNOTSUPP || e == rustix::io::Errno::NOSYS => {
            file.set_len(len)?;
            Ok(Allocation::Sparse)
        }
        Err(e) => Err(io::Error::from(e)),
    }
}

/// See [`crate::Platform::sync_dir`].
pub(crate) fn sync_dir(path: &Path) -> Result<(), PlatformError> {
    let fd = open_dir_nofollow(CWD, path, path)?;
    rustix::fs::fsync(&fd).map_err(errno(path))
}
