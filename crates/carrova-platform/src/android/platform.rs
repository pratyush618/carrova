//! [`AndroidPlatform`]: everything lives under the app's private files
//! directory; methods Android cannot serve keep the [`Platform`] defaults
//! (`watch_clipboard` too: Android hands the clipboard only to the app in
//! the foreground).

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::files::{self, MediaStoreSink};
use super::{clipboard, net, ring, scan, service};
use crate::sys;
use crate::{
    Allocation, ClipContent, DirEntry, DiskInfo, DownloadSink, FileLock, FileMeta, FsId,
    LanInterface, LinkNotice, MulticastGuard, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot,
    SnapshotCapability, SocketBuffers,
};

/// Keepalive of a link: a phone radio sleeps longer between packets.
const LINK_KEEPALIVE: Duration = Duration::from_secs(45);

/// Adapter rooted at the app's files directory.
#[derive(Debug, Clone)]
pub(crate) struct AndroidPlatform {
    /// App files directory (`Context.getFilesDir()`); `None` if Android
    /// did not say.
    files_dir: Option<OsString>,
    path: Option<OsString>,
}

impl AndroidPlatform {
    pub(crate) fn from_env() -> Self {
        Self {
            files_dir: files::files_dir().ok().map(PathBuf::into_os_string),
            path: std::env::var_os("PATH"),
        }
    }

    fn files_dir(&self) -> Result<PathBuf, PlatformError> {
        sys::home_dir(self.files_dir.as_deref())
    }
}

impl Platform for AndroidPlatform {
    fn os(&self) -> OsKind {
        OsKind::Android
    }

    /// Every root is a folder of the files directory; `Home` and
    /// `Documents` are the directory itself.
    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        let dir = self.files_dir()?;
        let rel = match root {
            SemanticRoot::Home | SemanticRoot::Documents => return Ok(dir),
            SemanticRoot::UserConfig => "config",
            SemanticRoot::UserData => "data",
            SemanticRoot::Cache => "cache",
        };
        sys::confine(root, &dir.join(rel))
    }

    fn state_dir(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.files_dir()?.join("state"))
    }

    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        Ok(sys::find_binary(self.path.as_deref(), name))
    }

    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        crate::meta::lstat(path)
    }

    /// Android hides other apps' processes.
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        Err(PlatformError::Unsupported("running_processes"))
    }

    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        sys::free_space(path)
    }

    fn snapshot_capability(&self, _path: &Path) -> Result<SnapshotCapability, PlatformError> {
        Ok(SnapshotCapability::None)
    }

    fn bind_local_socket(&self, path: &Path) -> Result<crate::LocalListener, PlatformError> {
        crate::LocalListener::bind(path, crate::netops::current_uid())
    }

    fn connect_local_socket(&self, path: &Path) -> Result<crate::LocalStream, PlatformError> {
        crate::LocalStream::connect(path)
    }

    fn disk(&self, path: &Path) -> Result<DiskInfo, PlatformError> {
        sys::disk(path)
    }

    fn open_read_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        sys::open_read_nofollow(path)
    }

    fn open_append_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        sys::open_append_nofollow(path)
    }

    fn open_rw_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        sys::open_rw_nofollow(path)
    }

    fn create_new_file(&self, path: &Path, mode: Option<u32>) -> Result<File, PlatformError> {
        sys::create_new_file(path, mode)
    }

    fn create_dir(&self, path: &Path, mode: Option<u32>) -> Result<(), PlatformError> {
        sys::create_dir(path, mode)
    }

    fn set_mode(&self, path: &Path, mode: u32) -> Result<(), PlatformError> {
        sys::set_mode(path, mode)
    }

    fn create_symlink(&self, target: &str, link: &Path) -> Result<(), PlatformError> {
        sys::create_symlink(target, link)
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        sys::read_link(path)
    }

    fn rename_noreplace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        sys::rename_noreplace(from, to)
    }

    fn rename_replace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        sys::rename_replace(from, to)
    }

    fn sync_dir(&self, path: &Path) -> Result<(), PlatformError> {
        sys::sync_dir(path)
    }

    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        sys::remove_file(path)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        sys::read_dir(path)
    }

    fn remove_dir_all_confined(&self, path: &Path, within: &Path) -> Result<(), PlatformError> {
        sys::remove_dir_all_confined(path, within)
    }

    fn fs_id(&self, path: &Path) -> Result<FsId, PlatformError> {
        sys::fs_id(path)
    }

    fn sync_fs(&self, path: &Path) -> Result<(), PlatformError> {
        sys::sync_fs(path)
    }

    fn write_all_at(&self, file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
        sys::write_all_at(file, buf, offset)
    }

    fn preallocate(&self, file: &File, len: u64) -> io::Result<Allocation> {
        sys::preallocate(file, len)
    }

    fn file_metadata(&self, file: &File) -> io::Result<FileMeta> {
        crate::meta::fstat(file)
    }

    fn lock_exclusive(&self, path: &Path) -> Result<FileLock, PlatformError> {
        sys::lock_exclusive(path)
    }

    fn try_lock_exclusive(&self, path: &Path) -> Result<Option<FileLock>, PlatformError> {
        sys::try_lock_exclusive(path)
    }

    fn current_uid(&self) -> Option<u32> {
        Some(crate::netops::current_uid())
    }

    fn device_name(&self) -> Option<String> {
        crate::device::sanitize_device_name(&crate::netops::host_name())
    }

    fn lan_ipv4_interfaces(&self) -> Result<Vec<LanInterface>, PlatformError> {
        crate::netops::lan_ipv4_interfaces()
    }

    /// Only Wi-Fi or Ethernet: mobile data has LAN-looking addresses too.
    fn on_local_network(&self) -> Result<bool, PlatformError> {
        net::on_local_network()
    }

    fn bind_broadcast_udp(&self, port: u16) -> io::Result<UdpSocket> {
        crate::netops::bind_broadcast_udp(port)
    }

    fn bind_udp(&self, port: u16, buffer: usize) -> io::Result<(UdpSocket, SocketBuffers)> {
        crate::netops::bind_udp(port, buffer)
    }

    fn send_magic_packet(&self, mac: [u8; 6]) -> Result<(), PlatformError> {
        crate::netops::send_magic_packet(mac)
    }

    fn clipboard_read(&self) -> Result<Option<ClipContent>, PlatformError> {
        clipboard::read()
    }

    fn clipboard_write(&self, content: &ClipContent) -> Result<(), PlatformError> {
        clipboard::write(content)
    }

    fn downloads_dir(&self) -> Result<PathBuf, PlatformError> {
        files::downloads_dir()
    }

    fn downloads_sink(
        &self,
        name: &str,
        mime: &str,
    ) -> Result<Box<dyn DownloadSink>, PlatformError> {
        Ok(Box::new(MediaStoreSink::create(name, mime)?))
    }

    fn open_url(&self, url: &str) -> Result<(), PlatformError> {
        files::open_url(crate::open::checked_url(url)?)
    }

    fn open_path(&self, path: &Path) -> Result<(), PlatformError> {
        files::open_path(crate::open::checked_path(path)?)
    }

    fn ring(&self, seconds: u16) -> Result<(), PlatformError> {
        ring::ring(seconds)
    }

    fn stop_ring(&self) -> Result<(), PlatformError> {
        ring::stop_ring()
    }

    fn multicast_lock(&self) -> Result<MulticastGuard, PlatformError> {
        net::multicast_lock()
    }

    fn link_keepalive(&self) -> Duration {
        LINK_KEEPALIVE
    }

    fn keep_running(&self, notice: Option<&LinkNotice>) -> Result<(), PlatformError> {
        service::keep_running(notice)
    }

    fn links_paused(&self) -> bool {
        service::links_paused()
    }

    fn scan_qr(&self) -> Result<Option<String>, PlatformError> {
        scan::scan_qr()
    }
}
