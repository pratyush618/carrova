//! Linux adapter.
//!
//! All environment and kernel-interface inputs (env vars, `/proc`, mountinfo,
//! `/sys`) are snapshotted or injectable so the logic is testable against a
//! tempdir instead of the real machine.
//!
//! Semantic roots and the state dir are resolved in `roots`, mountinfo
//! parsing and snapshot detection live in `mounts`.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use crate::{
    DirEntry, DiskInfo, FileMeta, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot,
    SnapshotCapability,
};

mod mounts;
mod roots;
#[cfg(test)]
#[path = "../../tests/linux/unit.rs"]
mod tests;

/// Snapshot of the environment variables the adapter depends on.
#[derive(Debug, Clone, Default)]
pub(crate) struct LinuxEnv {
    pub(crate) home: Option<OsString>,
    pub(crate) xdg_config_home: Option<OsString>,
    pub(crate) xdg_data_home: Option<OsString>,
    pub(crate) xdg_cache_home: Option<OsString>,
    pub(crate) xdg_state_home: Option<OsString>,
    pub(crate) path: Option<OsString>,
}

impl LinuxEnv {
    fn from_process() -> Self {
        Self {
            home: std::env::var_os("HOME"),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
            xdg_cache_home: std::env::var_os("XDG_CACHE_HOME"),
            xdg_state_home: std::env::var_os("XDG_STATE_HOME"),
            path: std::env::var_os("PATH"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LinuxPlatform {
    env: LinuxEnv,
    proc_root: PathBuf,
    mountinfo: PathBuf,
    sys_root: PathBuf,
}

impl LinuxPlatform {
    pub(crate) fn from_env() -> Self {
        Self::new(LinuxEnv::from_process())
    }

    /// Adapter over an explicit environment, using the real `/proc` and `/sys`.
    pub(crate) fn new(env: LinuxEnv) -> Self {
        Self {
            env,
            proc_root: PathBuf::from("/proc"),
            mountinfo: PathBuf::from("/proc/self/mountinfo"),
            sys_root: PathBuf::from("/sys"),
        }
    }

    /// Override the procfs root (default `/proc`).
    #[cfg(test)]
    fn with_proc_root(mut self, p: impl Into<PathBuf>) -> Self {
        self.proc_root = p.into();
        self
    }

    /// Override the mountinfo file (default `/proc/self/mountinfo`).
    #[cfg(test)]
    fn with_mountinfo(mut self, p: impl Into<PathBuf>) -> Self {
        self.mountinfo = p.into();
        self
    }

    /// Override the sysfs root (default `/sys`).
    #[cfg(test)]
    fn with_sys_root(mut self, p: impl Into<PathBuf>) -> Self {
        self.sys_root = p.into();
        self
    }
}

impl Platform for LinuxPlatform {
    fn os(&self) -> OsKind {
        OsKind::Linux
    }

    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        self.root_path(root)
    }

    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        Ok(crate::sys::find_binary(self.env.path.as_deref(), name))
    }

    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        crate::meta::lstat(path)
    }

    fn file_metadata(&self, file: &std::fs::File) -> std::io::Result<FileMeta> {
        crate::meta::fstat(file)
    }

    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        let entries = std::fs::read_dir(&self.proc_root)
            .map_err(|e| PlatformError::io(&self.proc_root, e))?;
        let mut out = Vec::new();
        for entry in entries {
            // Entry errors mid-iteration: treat like a vanished process.
            let Ok(entry) = entry else { continue };
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<u32>().ok())
            else {
                continue;
            };
            let dir = entry.path();
            // Process may exit between readdir and here: skip it.
            let Ok(comm) = std::fs::read_to_string(dir.join("comm")) else {
                continue;
            };
            let name = comm.strip_suffix('\n').unwrap_or(&comm).to_owned();
            // Other users' / kernel threads' exe is unreadable: None.
            let exe = std::fs::read_link(dir.join("exe")).ok();
            out.push(ProcessInfo { pid, name, exe });
        }
        out.sort_by_key(|p| p.pid);
        Ok(out)
    }

    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        crate::sys::free_space(path)
    }

    fn disk(&self, path: &Path) -> Result<DiskInfo, PlatformError> {
        crate::sys::disk(path)
    }

    /// See [`LinuxPlatform::detect_snapshot_capability`].
    fn snapshot_capability(&self, path: &Path) -> Result<SnapshotCapability, PlatformError> {
        self.detect_snapshot_capability(path)
    }

    fn state_dir(&self) -> Result<PathBuf, PlatformError> {
        self.state_dir_impl()
    }

    fn open_read_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::sys::open_read_nofollow(path)
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

    fn open_append_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::sys::open_append_nofollow(path)
    }

    fn open_rw_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::sys::open_rw_nofollow(path)
    }

    fn fs_id(&self, path: &Path) -> Result<crate::FsId, PlatformError> {
        crate::sys::fs_id(path)
    }

    fn sync_fs(&self, path: &Path) -> Result<(), PlatformError> {
        crate::sys::sync_fs(path)
    }

    fn write_all_at(&self, file: &std::fs::File, buf: &[u8], offset: u64) -> io::Result<()> {
        crate::sys::write_all_at(file, buf, offset)
    }

    fn preallocate(&self, file: &std::fs::File, len: u64) -> io::Result<crate::Allocation> {
        crate::sys::preallocate(file, len)
    }

    fn lock_exclusive(&self, path: &Path) -> Result<crate::FileLock, PlatformError> {
        crate::sys::lock_exclusive(path)
    }

    fn current_uid(&self) -> Option<u32> {
        Some(crate::netops::current_uid())
    }

    fn device_name(&self) -> Option<String> {
        crate::device::sanitize_device_name(&crate::netops::host_name())
    }

    fn lan_ipv4_interfaces(&self) -> Result<Vec<crate::LanInterface>, PlatformError> {
        crate::netops::lan_ipv4_interfaces()
    }

    fn bind_broadcast_udp(&self, port: u16) -> io::Result<std::net::UdpSocket> {
        crate::netops::bind_broadcast_udp(port)
    }

    fn bind_udp(
        &self,
        port: u16,
        buffer: usize,
    ) -> io::Result<(std::net::UdpSocket, crate::SocketBuffers)> {
        crate::netops::bind_udp(port, buffer)
    }

    fn udp_buffer_hint(&self, wanted: usize) -> Option<String> {
        Some(crate::netops::udp_buffer_hint(wanted))
    }

    /// Always `<home>/.gnupg` (an inherited `$GNUPGHOME` is ignored).
    fn gnupg_home(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.home()?.join(".gnupg"))
    }

    fn tool_command(&self, bin: &Path) -> Result<std::process::Command, PlatformError> {
        Ok(crate::sys::tool_command(
            bin,
            &self.home()?,
            self.env.path.as_deref(),
        ))
    }
}
