//! Advisory exclusive file lock (`flock(2)`).

use std::fs::File;
use std::path::Path;

use rustix::fs::{FileType, FlockOperation, Mode, OFlags};

use super::{errno, file_type, invalid};
use crate::{FileLock, PlatformError};

/// Mode of a newly created lock file: private to the user.
const LOCK_FILE_MODE: Mode = Mode::RUSR.union(Mode::WUSR);

/// See [`crate::Platform::lock_exclusive`]. Locks are per open file
/// description, so two opens in one process exclude each other too.
///
/// Opened read-only (`flock` needs no write access): closing a writable
/// fd is a change to a directory watch (`IN_CLOSE_WRITE`), and every lock
/// taken would wake a watcher of the lock's directory.
pub(crate) fn lock_exclusive(path: &Path) -> Result<FileLock, PlatformError> {
    let flags =
        OFlags::RDONLY | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NOCTTY;
    // NONBLOCK: a FIFO planted at `path` must not hang the open.
    let fd =
        rustix::fs::open(path, flags | OFlags::NONBLOCK, LOCK_FILE_MODE).map_err(errno(path))?;
    if file_type(path, &fd)? != FileType::RegularFile {
        return Err(invalid(path, "not a regular file"));
    }
    rustix::fs::flock(&fd, FlockOperation::LockExclusive).map_err(errno(path))?;
    Ok(FileLock::held(File::from(fd)))
}
