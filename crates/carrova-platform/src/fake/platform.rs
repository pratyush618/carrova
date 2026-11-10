//! [`Platform`] implementation of [`FakePlatform`].

use std::path::{Path, PathBuf};

use super::FakePlatform;
use crate::{
    FileMeta, FsId, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot, SnapshotCapability,
};

impl Platform for FakePlatform {
    fn os(&self) -> OsKind {
        OsKind::Linux
    }

    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        self.roots
            .get(&root)
            .cloned()
            .ok_or_else(|| PlatformError::RootUnavailable {
                root,
                reason: "not configured in FakePlatform".into(),
            })
    }

    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        Ok(self.binaries.get(name).cloned())
    }

    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        crate::meta::lstat(path).map(|m| self.hooks.path_meta(path, m))
    }

    fn file_metadata(&self, file: &std::fs::File) -> std::io::Result<FileMeta> {
        crate::meta::fstat(file).map(|m| self.hooks.fd_meta(m))
    }

    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        Ok(self.processes.clone())
    }

    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        if let Some(bytes) = self.hooks.free_space() {
            return Ok(bytes);
        }
        Ok(self.fake_fs(path).map_or(self.free_space, |f| f.free))
    }

    fn snapshot_capability(&self, _path: &Path) -> Result<SnapshotCapability, PlatformError> {
        Ok(SnapshotCapability::None)
    }

    fn state_dir(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.state_dir.clone())
    }

    #[cfg(target_os = "linux")]
    fn open_read_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        let file = crate::fsops::open_read_nofollow(path)?;
        if let Some(hook) = &self.hooks.hooks.on_open {
            hook(path);
        }
        Ok(file)
    }

    #[cfg(target_os = "linux")]
    fn create_new_file(
        &self,
        path: &Path,
        mode: Option<u32>,
    ) -> Result<std::fs::File, PlatformError> {
        crate::fsops::create_new_file(path, mode)
    }

    #[cfg(target_os = "linux")]
    fn create_dir(&self, path: &Path, mode: Option<u32>) -> Result<(), PlatformError> {
        crate::fsops::create_dir(path, mode)
    }

    #[cfg(target_os = "linux")]
    fn set_mode(&self, path: &Path, mode: u32) -> Result<(), PlatformError> {
        crate::fsops::set_mode(path, mode)
    }

    #[cfg(target_os = "linux")]
    fn create_symlink(&self, target: &str, link: &Path) -> Result<(), PlatformError> {
        crate::fsops::create_symlink(target, link)
    }

    #[cfg(target_os = "linux")]
    fn read_link(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        crate::fsops::read_link(path)
    }

    #[cfg(target_os = "linux")]
    fn rename_noreplace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        if let Some(hook) = &self.hooks.hooks.on_rename {
            hook(from, to)?;
        }
        crate::fsops::rename_noreplace(from, to)
    }

    #[cfg(target_os = "linux")]
    fn rename_replace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        crate::fsops::rename_replace(from, to)
    }

    #[cfg(target_os = "linux")]
    fn sync_dir(&self, path: &Path) -> Result<(), PlatformError> {
        self.hooks.count_dir_sync();
        crate::fsops::sync_dir(path)
    }

    #[cfg(target_os = "linux")]
    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        crate::fsops::remove_file(path)
    }

    #[cfg(target_os = "linux")]
    fn read_dir(&self, path: &Path) -> Result<Vec<crate::DirEntry>, PlatformError> {
        crate::fsops::read_dir(path)
    }

    #[cfg(target_os = "linux")]
    fn remove_dir_all_confined(&self, path: &Path, within: &Path) -> Result<(), PlatformError> {
        crate::fsops::remove_dir_all_confined(path, within)
    }

    #[cfg(target_os = "linux")]
    fn open_append_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::fsops::open_append_nofollow(path)
    }

    #[cfg(target_os = "linux")]
    fn lock_exclusive(&self, path: &Path) -> Result<crate::FileLock, PlatformError> {
        crate::fsops::lock_exclusive(path)
    }

    #[cfg(target_os = "linux")]
    fn open_rw_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::fsops::open_rw_nofollow(path)
    }

    #[cfg(target_os = "linux")]
    fn fs_id(&self, path: &Path) -> Result<FsId, PlatformError> {
        match self.fake_fs(path) {
            Some(f) => Ok(f.id),
            None => crate::fsops::fs_id(path),
        }
    }

    #[cfg(target_os = "linux")]
    fn sync_fs(&self, path: &Path) -> Result<(), PlatformError> {
        crate::fsops::sync_fs(path)
    }

    #[cfg(target_os = "linux")]
    fn write_all_at(&self, file: &std::fs::File, buf: &[u8], offset: u64) -> std::io::Result<()> {
        crate::fsops::write_all_at(file, buf, offset)
    }

    /// Real allocation, but more than the configured default free space
    /// fails with `StorageFull` (see [`FakePlatform::free_space_bytes`]).
    #[cfg(target_os = "linux")]
    fn preallocate(&self, file: &std::fs::File, len: u64) -> std::io::Result<crate::Allocation> {
        if len > self.free_space {
            return Err(std::io::Error::new(
                std::io::ErrorKind::StorageFull,
                "FakePlatform: allocation exceeds configured free space",
            ));
        }
        crate::fsops::preallocate(file, len)
    }

    /// The configured name ([`FakePlatform::with_device_name`]), sanitised.
    fn device_name(&self) -> Option<String> {
        self.device_name
            .as_deref()
            .and_then(crate::device::sanitize_device_name)
    }

    /// [`Hooks::uid`](super::Hooks::uid), else the real effective uid: fake metadata is real,
    /// so files the test creates are owned by it.
    #[cfg(target_os = "linux")]
    fn current_uid(&self) -> Option<u32> {
        self.hooks
            .hooks
            .uid
            .or_else(|| Some(crate::netops::current_uid()))
    }

    /// Real interfaces, like the Linux adapter (loopback tests need real
    /// sockets; discovery always also targets loopback).
    #[cfg(target_os = "linux")]
    fn lan_ipv4_interfaces(&self) -> Result<Vec<crate::LanInterface>, PlatformError> {
        crate::netops::lan_ipv4_interfaces()
    }

    #[cfg(target_os = "linux")]
    fn bind_broadcast_udp(&self, port: u16) -> std::io::Result<std::net::UdpSocket> {
        crate::netops::bind_broadcast_udp(port)
    }

    #[cfg(target_os = "linux")]
    fn bind_udp(
        &self,
        port: u16,
        buffer: usize,
    ) -> std::io::Result<(std::net::UdpSocket, crate::SocketBuffers)> {
        crate::netops::bind_udp(port, buffer)
    }

    #[cfg(target_os = "linux")]
    fn udp_buffer_hint(&self, wanted: usize) -> Option<String> {
        Some(crate::netops::udp_buffer_hint(wanted))
    }

    /// `<home root>/.gnupg`, like the Linux adapter.
    fn gnupg_home(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.resolve_root(SemanticRoot::Home)?.join(".gnupg"))
    }

    /// Same minimal environment as the Linux adapter, with the configured
    /// home root and the test process's `PATH` (fake tool scripts need
    /// `mkdir` etc.).
    #[cfg(target_os = "linux")]
    fn tool_command(&self, bin: &Path) -> Result<std::process::Command, PlatformError> {
        let path = std::env::var_os("PATH");
        Ok(crate::fsops::tool_command(
            bin,
            &self.resolve_root(SemanticRoot::Home)?,
            path.as_deref(),
        ))
    }
}
