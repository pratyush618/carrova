//! Windows adapter.
//!
//! Roots and the state dir come from known folders (`roots`), filesystem
//! primitives from `fs` (shared with the fake via `crate::sys`), LAN
//! networking from `netops`. Processes and free space use `sysinfo`.
//! Everything here is safe std or safe-wrapper crates: no FFI.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

use crate::{
    Allocation, DirEntry, DiskInfo, FileLock, FileMeta, FsId, LanInterface, OsKind, Platform,
    PlatformError, ProcessInfo, SemanticRoot, SnapshotCapability, SocketBuffers,
};

pub(crate) mod fs;
mod roots;
#[cfg(test)]
#[path = "../../tests/windows/unit.rs"]
mod tests;

/// Extensions `PATHEXT` has when unset (the Windows default subset that
/// can start a program directly).
const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// `read` of the volume whose mount point is the longest prefix of `path`.
///
/// shortcut: `path` is matched lexically, so a junction or `subst` drive
/// reports the volume it appears on, not the one it points to; resolve
/// the path first if staging ever lives behind one.
fn volume<T>(path: &Path, read: impl FnOnce(&sysinfo::Disk) -> T) -> Result<T, PlatformError> {
    let disks = sysinfo::Disks::new_with_refreshed_list()
        .map_err(|e| PlatformError::io(path, io::Error::other(e.to_string())))?;
    disks
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().components().count())
        .map(read)
        .ok_or_else(|| {
            PlatformError::io(path, io::Error::new(io::ErrorKind::NotFound, "no volume"))
        })
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct WindowsPlatform;

/// First `dir\name<ext>` (for each absolute `dir` in `path_var` and each
/// `ext` in `pathext`) that is a file, following links. A `name` with an
/// extension is also tried as-is. Names with separators, drive colons or
/// `.`/`..` are refused.
fn search_path(path_var: &OsStr, pathext: &str, name: &str) -> Option<PathBuf> {
    let bad = |c: char| matches!(c, '/' | '\\' | ':');
    if name.is_empty() || name == "." || name == ".." || name.contains(bad) {
        return None;
    }
    let has_ext = Path::new(name).extension().is_some();
    let names: Vec<String> = has_ext
        .then(|| name.to_owned())
        .into_iter()
        .chain(
            pathext
                .split(';')
                .filter(|e| !e.is_empty())
                .map(|e| format!("{name}{e}")),
        )
        .collect();
    std::env::split_paths(path_var)
        .filter(|dir| dir.is_absolute())
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|candidate| std::fs::metadata(candidate).is_ok_and(|m| m.is_file()))
}

/// Process name as recipes write it: the executable's file stem
/// (`Code.exe` → `Code`).
fn process_name(image: &OsStr) -> String {
    Path::new(image)
        .file_stem()
        .unwrap_or(image)
        .to_string_lossy()
        .into_owned()
}

impl Platform for WindowsPlatform {
    fn os(&self) -> OsKind {
        OsKind::Windows
    }

    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        roots::root_path(root)
    }

    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        let Some(path_var) = std::env::var_os("PATH") else {
            return Ok(None);
        };
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| DEFAULT_PATHEXT.into());
        Ok(search_path(&path_var, &pathext, name))
    }

    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        crate::meta::lstat(path)
    }

    fn file_metadata(&self, file: &std::fs::File) -> io::Result<FileMeta> {
        crate::meta::fstat(file)
    }

    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        use sysinfo::{ProcessRefreshKind, RefreshKind, System, UpdateKind};
        let refresh = RefreshKind::nothing()
            .with_processes(ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet));
        let sys = System::new_with_specifics(refresh)
            .map_err(|e| PlatformError::io("processes", io::Error::other(e.to_string())))?;
        let mut out: Vec<ProcessInfo> = sys
            .processes()
            .iter()
            .map(|(pid, p)| ProcessInfo {
                pid: pid.as_u32(),
                name: process_name(p.name()),
                exe: p.exe().map(Path::to_path_buf),
            })
            .collect();
        out.sort_by_key(|p| p.pid);
        Ok(out)
    }

    /// Free space of the volume holding `path` (see [`volume`]).
    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        volume(path, sysinfo::Disk::available_space)
    }

    /// Capacity and root of the volume holding `path` (see [`volume`]).
    fn disk(&self, path: &Path) -> Result<DiskInfo, PlatformError> {
        volume(path, |d| DiskInfo {
            total: d.total_space(),
            mount_point: d.mount_point().to_path_buf(),
        })
    }

    fn snapshot_capability(&self, _path: &Path) -> Result<SnapshotCapability, PlatformError> {
        Ok(SnapshotCapability::None)
    }

    fn state_dir(&self) -> Result<PathBuf, PlatformError> {
        roots::state_dir()
    }

    fn open_read_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::sys::open_read_nofollow(path)
    }

    fn open_append_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::sys::open_append_nofollow(path)
    }

    fn create_new_file(
        &self,
        path: &Path,
        mode: Option<u32>,
    ) -> Result<std::fs::File, PlatformError> {
        crate::sys::create_new_file(path, mode)
    }

    fn create_dir(&self, path: &Path, mode: Option<u32>) -> Result<(), PlatformError> {
        crate::sys::create_dir(path, mode)
    }

    fn set_mode(&self, path: &Path, mode: u32) -> Result<(), PlatformError> {
        crate::sys::set_mode(path, mode)
    }

    fn create_symlink(&self, target: &str, link: &Path) -> Result<(), PlatformError> {
        crate::sys::create_symlink(target, link)
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        crate::sys::read_link(path)
    }

    fn rename_noreplace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        crate::sys::rename_noreplace(from, to)
    }

    fn rename_replace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        crate::sys::rename_replace(from, to)
    }

    fn sync_dir(&self, path: &Path) -> Result<(), PlatformError> {
        crate::sys::sync_dir(path)
    }

    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        crate::sys::remove_file(path)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        crate::sys::read_dir(path)
    }

    fn remove_dir_all_confined(&self, path: &Path, within: &Path) -> Result<(), PlatformError> {
        crate::sys::remove_dir_all_confined(path, within)
    }

    fn open_rw_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::sys::open_rw_nofollow(path)
    }

    fn fs_id(&self, path: &Path) -> Result<FsId, PlatformError> {
        crate::sys::fs_id(path)
    }

    fn sync_fs(&self, path: &Path) -> Result<(), PlatformError> {
        crate::sys::sync_fs(path)
    }

    fn write_all_at(&self, file: &std::fs::File, buf: &[u8], offset: u64) -> io::Result<()> {
        crate::sys::write_all_at(file, buf, offset)
    }

    fn preallocate(&self, file: &std::fs::File, len: u64) -> io::Result<Allocation> {
        crate::sys::preallocate(file, len)
    }

    fn lock_exclusive(&self, path: &Path) -> Result<FileLock, PlatformError> {
        crate::sys::lock_exclusive(path)
    }

    /// The NetBIOS computer name (what `%COMPUTERNAME%` holds), read from
    /// the OS rather than the inheritable environment.
    fn device_name(&self) -> Option<String> {
        use winapi_util::sysinfo::{ComputerNameKind, get_computer_name};
        let name = get_computer_name(ComputerNameKind::NetBios).ok()?;
        crate::device::sanitize_device_name(&name.to_string_lossy())
    }

    fn lan_ipv4_interfaces(&self) -> Result<Vec<LanInterface>, PlatformError> {
        crate::netops::lan_ipv4_interfaces()
    }

    fn bind_broadcast_udp(&self, port: u16) -> io::Result<std::net::UdpSocket> {
        crate::netops::bind_broadcast_udp(port)
    }

    fn bind_udp(
        &self,
        port: u16,
        buffer: usize,
    ) -> io::Result<(std::net::UdpSocket, SocketBuffers)> {
        crate::netops::bind_udp(port, buffer)
    }

    fn gnupg_home(&self) -> Result<PathBuf, PlatformError> {
        roots::gnupg_home()
    }

    /// `USERPROFILE`, `APPDATA`, `LOCALAPPDATA` from the resolved roots,
    /// plus `PATH`, `PATHEXT`, `SystemRoot`, `TEMP`; nothing else inherited.
    fn tool_command(&self, bin: &Path) -> Result<std::process::Command, PlatformError> {
        let home = roots::root_path(SemanticRoot::Home)?;
        let path: Option<OsString> = std::env::var_os("PATH");
        let mut cmd = crate::sys::tool_command(bin, &home, path.as_deref());
        cmd.env("APPDATA", roots::root_path(SemanticRoot::UserConfig)?)
            .env("LOCALAPPDATA", roots::root_path(SemanticRoot::UserData)?);
        Ok(cmd)
    }
}
