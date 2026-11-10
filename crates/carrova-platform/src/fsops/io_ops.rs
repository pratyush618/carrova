//! Durability and positional I/O: filesystem ids, syncs, preallocation.

use std::fs::File;
use std::io;
use std::path::Path;

use rustix::fs::CWD;

use super::dir::open_dir_nofollow;
use super::errno;
use crate::{Allocation, DiskInfo, FsId, PlatformError};

/// See [`crate::Platform::fs_id`].
pub(crate) fn fs_id(path: &Path) -> Result<FsId, PlatformError> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).map_err(|e| PlatformError::io(path, e))?;
    Ok(FsId::new(meta.dev()))
}

/// See [`crate::Platform::free_space`]: `statvfs(2)` blocks available to
/// unprivileged users.
pub(crate) fn free_space(path: &Path) -> Result<u64, PlatformError> {
    let st = rustix::fs::statvfs(path).map_err(errno(path))?;
    Ok(st.f_bavail.saturating_mul(st.f_frsize))
}

/// See [`crate::Platform::disk`]: `statvfs(2)` capacity; the mount point
/// is the highest ancestor of the resolved path still on its device.
pub(crate) fn disk(path: &Path) -> Result<DiskInfo, PlatformError> {
    use std::os::unix::fs::MetadataExt;
    let dev = |p: &Path| {
        std::fs::metadata(p)
            .map(|m| m.dev())
            .map_err(|e| PlatformError::io(p, e))
    };
    let st = rustix::fs::statvfs(path).map_err(errno(path))?;
    let real = std::fs::canonicalize(path).map_err(|e| PlatformError::io(path, e))?;
    let own = dev(&real)?;
    let mut mount_point = real.as_path();
    while let Some(parent) = mount_point.parent() {
        if dev(parent)? != own {
            break;
        }
        mount_point = parent;
    }
    Ok(DiskInfo {
        total: st.f_blocks.saturating_mul(st.f_frsize),
        mount_point: mount_point.to_path_buf(),
    })
}

/// See [`crate::Platform::sync_fs`]: `syncfs(2)` on an `O_NOFOLLOW` fd of
/// `path` (a directory or regular file).
#[cfg(target_os = "linux")]
pub(crate) fn sync_fs(path: &Path) -> Result<(), PlatformError> {
    use rustix::fs::{FileType, Mode, OFlags};

    use super::{file_type, invalid};

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

/// See [`crate::Platform::sync_fs`]: there is no `syncfs(2)`, and an
/// `fsync` would flush one file only, so callers must sync per file.
#[cfg(not(target_os = "linux"))]
pub(crate) fn sync_fs(_path: &Path) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported("sync_fs"))
}

/// See [`crate::Platform::write_all_at`].
pub(crate) fn write_all_at(file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::write_all_at(file, buf, offset)
}

/// See [`crate::Platform::preallocate`]: `fallocate(2)` (mode 0; macOS
/// `F_PREALLOCATE` + `ftruncate`) reserves the blocks, so a full disk fails
/// here (`StorageFull`) instead of mid-write. Filesystems without it get a
/// sparse `ftruncate`.
pub(crate) fn preallocate(file: &File, len: u64) -> io::Result<Allocation> {
    if len == 0 {
        file.set_len(0)?;
        return Ok(Allocation::Reserved);
    }
    match rustix::fs::fallocate(file, rustix::fs::FallocateFlags::empty(), 0, len) {
        Ok(()) => Ok(Allocation::Reserved),
        // macOS reports ENOTSUP, which differs from EOPNOTSUPP there.
        Err(e)
            if e == rustix::io::Errno::OPNOTSUPP
                || e == rustix::io::Errno::NOTSUP
                || e == rustix::io::Errno::NOSYS =>
        {
            file.set_len(len)?;
            Ok(Allocation::Sparse)
        }
        Err(e) => Err(io::Error::from(e)),
    }
}

/// See [`crate::Platform::sync_dir`].
pub(crate) fn sync_dir(path: &Path) -> Result<(), PlatformError> {
    let fd = open_dir_nofollow(CWD, path, path)?;
    // macOS `fsync` stops at the drive cache; `F_FULLFSYNC` is what std's
    // `File::sync_all` uses there.
    #[cfg(target_os = "macos")]
    let synced = rustix::fs::fcntl_fullfsync(&fd);
    #[cfg(not(target_os = "macos"))]
    let synced = rustix::fs::fsync(&fd);
    synced.map_err(errno(path))
}
