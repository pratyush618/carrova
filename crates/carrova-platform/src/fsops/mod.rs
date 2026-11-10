//! Safe filesystem primitives and per-user environment (Unix: Linux and
//! macOS), shared by the native adapters and the fake platform.
//!
//! Every operation acts on the final path component **without following
//! symlinks** and reports failures as [`PlatformError::Io`] carrying the
//! original [`io::ErrorKind`] (e.g. `AlreadyExists`, `CrossesDevices`).
//! Modes are applied with `fchmod`/`chmod` on an fd after creation, so the
//! process umask never affects the final permission bits.

use std::io;
use std::os::fd::AsFd;
use std::path::Path;

use rustix::fs::{FileType, Mode, RawMode};

use crate::PlatformError;

mod create;
mod dir;
mod env;
mod io_ops;
mod lock;
mod open;
mod remove;
mod rename;

pub(crate) use create::{create_dir, create_new_file, create_symlink, set_mode};
pub(crate) use dir::read_dir;
pub(crate) use env::{confine, find_binary, home_dir, tool_command};
pub(crate) use io_ops::{free_space, fs_id, preallocate, sync_dir, sync_fs, write_all_at};
pub(crate) use lock::lock_exclusive;
pub(crate) use open::{open_append_nofollow, open_read_nofollow, open_rw_nofollow, read_link};
pub(crate) use remove::{remove_dir_all_confined, remove_file};
pub(crate) use rename::{rename_noreplace, rename_replace};

/// Permission bits a caller may set; setuid/setgid/sticky are refused.
const PERM_MASK: u32 = 0o777;

fn errno(path: &Path) -> impl FnOnce(rustix::io::Errno) -> PlatformError + '_ {
    move |e| PlatformError::io(path, io::Error::from(e))
}

fn invalid(path: &Path, msg: &'static str) -> PlatformError {
    PlatformError::io(path, io::Error::new(io::ErrorKind::InvalidInput, msg))
}

fn perm(path: &Path, mode: u32) -> Result<Mode, PlatformError> {
    // `mode_t` is narrower than u32 on macOS; `0o777` fits everywhere.
    match RawMode::try_from(mode) {
        Ok(raw) if mode & !PERM_MASK == 0 => Ok(Mode::from_raw_mode(raw)),
        _ => Err(invalid(path, "mode has bits outside 0o777")),
    }
}

fn file_type(path: &Path, fd: impl AsFd) -> Result<FileType, PlatformError> {
    let st = rustix::fs::fstat(fd).map_err(errno(path))?;
    Ok(FileType::from_raw_mode(st.st_mode))
}

#[cfg(test)]
#[path = "../../tests/fsops/unit.rs"]
mod tests;
