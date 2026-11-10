//! [`Platform`] implementation of [`FakePlatform`].

use std::path::{Path, PathBuf};

use super::FakePlatform;
use crate::{
    FileMeta, FsId, OsKind, PermissionModel, Platform, PlatformError, ProcessInfo, SemanticRoot,
    SnapshotCapability,
};

impl Platform for FakePlatform {
    fn os(&self) -> OsKind {
        self.os
    }

    /// Follows [`Platform::os`] on a Unix host. A Windows host's real files
    /// carry no mode bits, so there it is always ACL, whatever `os`.
    fn permission_model(&self) -> PermissionModel {
        if cfg!(unix) {
            self.os.permission_model()
        } else {
            PermissionModel::Acl
        }
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

    #[cfg(any(unix, windows))]
    fn open_read_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        let file = crate::sys::open_read_nofollow(path)?;
        if let Some(hook) = &self.hooks.hooks.on_open {
            hook(path);
        }
        Ok(file)
    }

    #[cfg(any(unix, windows))]
    fn create_new_file(
        &self,
        path: &Path,
        mode: Option<u32>,
    ) -> Result<std::fs::File, PlatformError> {
        crate::sys::create_new_file(path, mode)
    }

    #[cfg(any(unix, windows))]
    fn create_dir(&self, path: &Path, mode: Option<u32>) -> Result<(), PlatformError> {
        crate::sys::create_dir(path, mode)
    }

    #[cfg(any(unix, windows))]
    fn set_mode(&self, path: &Path, mode: u32) -> Result<(), PlatformError> {
        crate::sys::set_mode(path, mode)
    }

    #[cfg(any(unix, windows))]
    fn create_symlink(&self, target: &str, link: &Path) -> Result<(), PlatformError> {
        crate::sys::create_symlink(target, link)
    }

    #[cfg(any(unix, windows))]
    fn read_link(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        crate::sys::read_link(path)
    }

    #[cfg(any(unix, windows))]
    fn rename_noreplace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        if let Some(hook) = &self.hooks.hooks.on_rename {
            hook(from, to)?;
        }
        crate::sys::rename_noreplace(from, to)
    }

    #[cfg(any(unix, windows))]
    fn rename_replace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        crate::sys::rename_replace(from, to)
    }

    #[cfg(any(unix, windows))]
    fn sync_dir(&self, path: &Path) -> Result<(), PlatformError> {
        self.hooks.count_dir_sync();
        crate::sys::sync_dir(path)
    }

    #[cfg(any(unix, windows))]
    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        crate::sys::remove_file(path)
    }

    #[cfg(any(unix, windows))]
    fn read_dir(&self, path: &Path) -> Result<Vec<crate::DirEntry>, PlatformError> {
        crate::sys::read_dir(path)
    }

    #[cfg(any(unix, windows))]
    fn remove_dir_all_confined(&self, path: &Path, within: &Path) -> Result<(), PlatformError> {
        crate::sys::remove_dir_all_confined(path, within)
    }

    #[cfg(any(unix, windows))]
    fn open_append_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::sys::open_append_nofollow(path)
    }

    #[cfg(any(unix, windows))]
    fn lock_exclusive(&self, path: &Path) -> Result<crate::FileLock, PlatformError> {
        crate::sys::lock_exclusive(path)
    }

    #[cfg(any(unix, windows))]
    fn open_rw_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        crate::sys::open_rw_nofollow(path)
    }

    #[cfg(any(unix, windows))]
    fn fs_id(&self, path: &Path) -> Result<FsId, PlatformError> {
        match self.fake_fs(path) {
            Some(f) => Ok(f.id),
            None => crate::sys::fs_id(path),
        }
    }

    #[cfg(any(unix, windows))]
    fn sync_fs(&self, path: &Path) -> Result<(), PlatformError> {
        crate::sys::sync_fs(path)
    }

    #[cfg(any(unix, windows))]
    fn write_all_at(&self, file: &std::fs::File, buf: &[u8], offset: u64) -> std::io::Result<()> {
        crate::sys::write_all_at(file, buf, offset)
    }

    /// Real allocation, but more than the configured default free space
    /// fails with `StorageFull` (see [`FakePlatform::free_space_bytes`]).
    #[cfg(any(unix, windows))]
    fn preallocate(&self, file: &std::fs::File, len: u64) -> std::io::Result<crate::Allocation> {
        if len > self.free_space {
            return Err(std::io::Error::new(
                std::io::ErrorKind::StorageFull,
                "FakePlatform: allocation exceeds configured free space",
            ));
        }
        crate::sys::preallocate(file, len)
    }

    /// The configured name ([`FakePlatform::with_device_name`]), sanitised.
    fn device_name(&self) -> Option<String> {
        self.device_name
            .as_deref()
            .and_then(crate::device::sanitize_device_name)
    }

    /// [`Hooks::uid`](super::Hooks::uid), else the real effective uid: fake metadata is real,
    /// so files the test creates are owned by it.
    #[cfg(unix)]
    fn current_uid(&self) -> Option<u32> {
        self.hooks
            .hooks
            .uid
            .or_else(|| Some(crate::netops::current_uid()))
    }

    /// Real interfaces, like the native adapters (loopback tests need real
    /// sockets; discovery always also targets loopback).
    #[cfg(any(unix, windows))]
    fn lan_ipv4_interfaces(&self) -> Result<Vec<crate::LanInterface>, PlatformError> {
        crate::netops::lan_ipv4_interfaces()
    }

    #[cfg(any(unix, windows))]
    fn bind_broadcast_udp(&self, port: u16) -> std::io::Result<std::net::UdpSocket> {
        crate::netops::bind_broadcast_udp(port)
    }

    #[cfg(any(unix, windows))]
    fn bind_udp(
        &self,
        port: u16,
        buffer: usize,
    ) -> std::io::Result<(std::net::UdpSocket, crate::SocketBuffers)> {
        crate::netops::bind_udp(port, buffer)
    }

    #[cfg(unix)]
    fn udp_buffer_hint(&self, wanted: usize) -> Option<String> {
        Some(crate::netops::udp_buffer_hint(wanted))
    }

    /// `<home root>/.gnupg`, like the Unix adapters.
    fn gnupg_home(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.resolve_root(SemanticRoot::Home)?.join(".gnupg"))
    }

    /// Same minimal environment as the native adapter, with the configured
    /// home root and the test process's `PATH` (fake tool scripts need
    /// `mkdir` etc.).
    #[cfg(any(unix, windows))]
    fn tool_command(&self, bin: &Path) -> Result<std::process::Command, PlatformError> {
        let path = std::env::var_os("PATH");
        Ok(crate::sys::tool_command(
            bin,
            &self.resolve_root(SemanticRoot::Home)?,
            path.as_deref(),
        ))
    }
}
