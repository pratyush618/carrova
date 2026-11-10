//! Creating files, directories and symlinks with exact modes.

use std::fs::File;
use std::io;
use std::path::Path;

use rustix::fs::{FileType, Mode, OFlags};

use super::{errno, invalid, perm};
use crate::PlatformError;

/// See [`crate::Platform::create_new_file`].
pub(crate) fn create_new_file(path: &Path, mode: Option<u32>) -> Result<File, PlatformError> {
    let exact = mode.map(|m| perm(path, m)).transpose()?;
    let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    // Create no more permissive than requested; fchmod then sets it exactly.
    let initial = exact.unwrap_or(Mode::from_raw_mode(0o666));
    let fd = rustix::fs::open(path, flags, initial).map_err(errno(path))?;
    if let Some(m) = exact {
        rustix::fs::fchmod(&fd, m).map_err(errno(path))?;
    }
    Ok(File::from(fd))
}

/// See [`crate::Platform::create_dir`].
pub(crate) fn create_dir(path: &Path, mode: Option<u32>) -> Result<(), PlatformError> {
    let exact = mode.map(|m| perm(path, m)).transpose()?;
    let initial = exact.unwrap_or(Mode::from_raw_mode(0o777));
    rustix::fs::mkdir(path, initial).map_err(errno(path))?;
    if let Some(m) = exact {
        chmod_nofollow(path, m, Some(FileType::Directory))?;
    }
    Ok(())
}

/// See [`crate::Platform::set_mode`].
pub(crate) fn set_mode(path: &Path, mode: u32) -> Result<(), PlatformError> {
    chmod_nofollow(path, perm(path, mode)?, None)
}

/// chmod the object at `path` itself, refusing symlinks and anything other
/// than a regular file or directory (or exactly `expect`, if given).
///
/// Uses an `O_PATH|O_NOFOLLOW` fd (no read permission needed, no open side
/// effects on special files), checks its type with `fstat`, then chmods the
/// inode it pins via `/proc/self/fd/N` — `fchmod` is not allowed on `O_PATH`
/// fds, and `fchmodat(AT_SYMLINK_NOFOLLOW)` is not portable across kernels.
#[cfg(target_os = "linux")]
fn chmod_nofollow(path: &Path, mode: Mode, expect: Option<FileType>) -> Result<(), PlatformError> {
    use std::os::fd::AsRawFd;

    let fd = rustix::fs::open(
        path,
        OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(errno(path))?;
    check_chmod_target(path, super::file_type(path, &fd)?, expect)?;
    let magic = format!("/proc/self/fd/{}", fd.as_raw_fd());
    rustix::fs::chmod(magic.as_str(), mode).map_err(errno(path))
}

/// chmod the object at `path` itself, refusing symlinks and anything other
/// than a regular file or directory (or exactly `expect`, if given).
///
/// No `O_PATH` here: `lstat` checks the type, then
/// `fchmodat(AT_SYMLINK_NOFOLLOW)` changes whatever is at `path` without
/// following it, so a swap in between can at worst chmod a link itself,
/// never its target.
#[cfg(not(target_os = "linux"))]
fn chmod_nofollow(path: &Path, mode: Mode, expect: Option<FileType>) -> Result<(), PlatformError> {
    use rustix::fs::{AtFlags, CWD};

    let st = rustix::fs::statat(CWD, path, AtFlags::SYMLINK_NOFOLLOW).map_err(errno(path))?;
    check_chmod_target(path, FileType::from_raw_mode(st.st_mode), expect)?;
    rustix::fs::chmodat(CWD, path, mode, AtFlags::SYMLINK_NOFOLLOW).map_err(errno(path))
}

fn check_chmod_target(
    path: &Path,
    actual: FileType,
    expect: Option<FileType>,
) -> Result<(), PlatformError> {
    match (actual, expect) {
        (FileType::Symlink, _) => Err(PlatformError::io(
            path,
            io::Error::from(rustix::io::Errno::LOOP),
        )),
        (actual, Some(want)) if actual != want => {
            Err(invalid(path, "object changed type during operation"))
        }
        (FileType::RegularFile | FileType::Directory, _) => Ok(()),
        _ => Err(invalid(path, "not a regular file or directory")),
    }
}

/// See [`crate::Platform::create_symlink`].
pub(crate) fn create_symlink(target: &str, link: &Path) -> Result<(), PlatformError> {
    std::os::unix::fs::symlink(target, link).map_err(|e| PlatformError::io(link, e))
}
