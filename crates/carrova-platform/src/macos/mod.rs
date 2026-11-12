//! macOS adapter.
//!
//! Filesystem primitives, `HOME`/`PATH` handling and tool environments are
//! the Unix ones shared with Linux (`crate::sys`); LAN sockets come from
//! `crate::netops`. Only roots, processes and a few answers differ.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use crate::{
    DirEntry, DiskInfo, FileManagerStatus, FileMeta, OsKind, Platform, PlatformError, ProcessInfo,
    SemanticRoot, SnapshotCapability,
};

mod background;
mod file_manager;
mod roots;
#[cfg(test)]
#[path = "../../tests/macos/unit.rs"]
mod tests;

/// Snapshot of the environment variables the adapter depends on.
#[derive(Debug, Clone, Default)]
pub(crate) struct MacEnv {
    pub(crate) home: Option<OsString>,
    pub(crate) path: Option<OsString>,
}

impl MacEnv {
    fn from_process() -> Self {
        Self {
            home: std::env::var_os("HOME"),
            path: std::env::var_os("PATH"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MacPlatform {
    env: MacEnv,
}

impl MacPlatform {
    pub(crate) fn from_env() -> Self {
        Self::new(MacEnv::from_process())
    }

    /// Adapter over an explicit environment.
    pub(crate) fn new(env: MacEnv) -> Self {
        Self { env }
    }
}

/// `kern.ipc.maxsockbuf` value under which a socket buffer of `wanted`
/// bytes is granted: the kernel caps buffers at `maxsockbuf * 2048 / 2304`
/// (mbuf cluster overhead), i.e. `wanted` needs `wanted * 9 / 8`.
fn maxsockbuf_for(wanted: usize) -> usize {
    // `wanted + wanted / 8` (rounded up): saturates at the top instead of
    // overflowing in the multiply.
    wanted.saturating_add(wanted.div_ceil(8))
}

impl Platform for MacPlatform {
    fn os(&self) -> OsKind {
        OsKind::MacOs
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

    fn file_metadata(&self, file: &std::fs::File) -> io::Result<FileMeta> {
        crate::meta::fstat(file)
    }

    /// Names are the executable's file name, untruncated (unlike Linux
    /// `comm`); `exe` is `None` for other users' processes.
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

        // Never fails on macOS (sysinfo only reads the mach host port).
        let mut sys = System::new().map_err(|_| PlatformError::Unsupported("running_processes"))?;
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let mut out: Vec<ProcessInfo> = sys
            .processes()
            .values()
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                exe: p.exe().map(Path::to_path_buf),
            })
            .collect();
        out.sort_by_key(|p| p.pid);
        Ok(out)
    }

    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        crate::sys::free_space(path)
    }

    fn disk(&self, path: &Path) -> Result<DiskInfo, PlatformError> {
        crate::sys::disk(path)
    }

    /// APFS snapshots need privileges Carrova does not ask for.
    fn snapshot_capability(&self, _path: &Path) -> Result<SnapshotCapability, PlatformError> {
        Ok(SnapshotCapability::None)
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

    /// Always [`PlatformError::Unsupported`]: macOS has no `syncfs`.
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

    /// The host name without Bonjour's `.local` suffix.
    // shortcut: host name, not the System Settings "Computer Name" (needs
    // SystemConfiguration FFI); upgrade if users find the name unfamiliar.
    fn device_name(&self) -> Option<String> {
        let host = crate::netops::host_name();
        crate::device::sanitize_device_name(host.strip_suffix(".local").unwrap_or(&host))
    }

    fn lan_ipv4_interfaces(&self) -> Result<Vec<crate::LanInterface>, PlatformError> {
        crate::netops::lan_ipv4_interfaces()
    }

    // shortcut: no neighbour lookup on macOS (needs `arp -an` or sysctl
    // PF_ROUTE); the default `None` stands until Wake is wanted there.
    fn send_magic_packet(&self, mac: [u8; 6]) -> Result<(), PlatformError> {
        crate::netops::send_magic_packet(mac)
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
        Some(format!(
            "raise the limit for full LAN speed, e.g. \
             `sudo sysctl -w kern.ipc.maxsockbuf={}`",
            maxsockbuf_for(wanted)
        ))
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

    fn file_manager_status(&self) -> Result<FileManagerStatus, PlatformError> {
        self.quick_action_status()
    }

    /// The Quick Action opens the `.app` bundle holding `launcher`;
    /// [`PlatformError::Unsupported`] when there is none (a dev build).
    fn install_file_manager_entries(
        &self,
        launcher: &Path,
    ) -> Result<FileManagerStatus, PlatformError> {
        self.install_quick_action(launcher)
    }

    fn remove_file_manager_entries(&self) -> Result<FileManagerStatus, PlatformError> {
        self.remove_quick_action()
    }

    fn autostart_status(&self) -> Result<bool, PlatformError> {
        self.launch_agent_on()
    }

    fn set_autostart(&self, launcher: &Path, on: bool) -> Result<bool, PlatformError> {
        self.write_launch_agent(launcher, on)
    }
}
