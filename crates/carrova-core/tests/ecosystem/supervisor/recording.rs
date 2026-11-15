//! [`FakePlatform`] that also records what the supervisor opens and rings
//! (the fake leaves `open_url`, `open_path` and `ring` unsupported).

use std::fs::File;
use std::io;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use carrova_platform::fake::FakePlatform;
use carrova_platform::{
    ClipContent, ClipboardWatch, DirEntry, DirWatch, FileLock, FileMeta, LanInterface,
    LocalListener, LocalStream, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot,
    SnapshotCapability, SocketBuffers,
};

/// What a [`Recording`] platform was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    OpenUrl(String),
    OpenPath(PathBuf),
    Ring(u16),
    StopRing,
}

#[derive(Debug)]
pub struct Recording {
    pub inner: FakePlatform,
    done: Mutex<Vec<Done>>,
    /// Plays an OS without local sockets (Windows, macOS today).
    no_local_socket: AtomicBool,
}

impl Recording {
    pub fn new(inner: FakePlatform) -> Self {
        Self {
            inner,
            done: Mutex::default(),
            no_local_socket: AtomicBool::new(false),
        }
    }

    pub fn refuse_local_sockets(&self) {
        self.no_local_socket.store(true, Ordering::SeqCst);
    }

    pub fn done(&self) -> Vec<Done> {
        self.done
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn record(&self, done: Done) -> Result<(), PlatformError> {
        self.done
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(done);
        Ok(())
    }
}

impl Platform for Recording {
    fn os(&self) -> OsKind {
        self.inner.os()
    }
    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        self.inner.resolve_root(root)
    }
    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        self.inner.find_binary(name)
    }
    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        self.inner.metadata(path)
    }
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        self.inner.running_processes()
    }
    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        self.inner.free_space(path)
    }
    fn snapshot_capability(&self, path: &Path) -> Result<SnapshotCapability, PlatformError> {
        self.inner.snapshot_capability(path)
    }
    fn state_dir(&self) -> Result<PathBuf, PlatformError> {
        self.inner.state_dir()
    }
    fn open_read_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        self.inner.open_read_nofollow(path)
    }
    fn open_append_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        self.inner.open_append_nofollow(path)
    }
    fn create_new_file(&self, path: &Path, mode: Option<u32>) -> Result<File, PlatformError> {
        self.inner.create_new_file(path, mode)
    }
    fn create_dir(&self, path: &Path, mode: Option<u32>) -> Result<(), PlatformError> {
        self.inner.create_dir(path, mode)
    }
    fn set_mode(&self, path: &Path, mode: u32) -> Result<(), PlatformError> {
        self.inner.set_mode(path, mode)
    }
    fn read_link(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        self.inner.read_link(path)
    }
    fn rename_noreplace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        self.inner.rename_noreplace(from, to)
    }
    fn rename_replace(&self, from: &Path, to: &Path) -> Result<(), PlatformError> {
        self.inner.rename_replace(from, to)
    }
    fn sync_dir(&self, path: &Path) -> Result<(), PlatformError> {
        self.inner.sync_dir(path)
    }
    fn remove_file(&self, path: &Path) -> Result<(), PlatformError> {
        self.inner.remove_file(path)
    }
    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        self.inner.read_dir(path)
    }
    fn remove_dir_all_confined(&self, path: &Path, within: &Path) -> Result<(), PlatformError> {
        self.inner.remove_dir_all_confined(path, within)
    }
    fn open_rw_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        self.inner.open_rw_nofollow(path)
    }
    fn write_all_at(&self, file: &File, buf: &[u8], offset: u64) -> io::Result<()> {
        self.inner.write_all_at(file, buf, offset)
    }
    fn file_metadata(&self, file: &File) -> io::Result<FileMeta> {
        self.inner.file_metadata(file)
    }
    fn lock_exclusive(&self, path: &Path) -> Result<FileLock, PlatformError> {
        self.inner.lock_exclusive(path)
    }
    fn try_lock_exclusive(&self, path: &Path) -> Result<Option<FileLock>, PlatformError> {
        self.inner.try_lock_exclusive(path)
    }
    fn bind_local_socket(&self, path: &Path) -> Result<LocalListener, PlatformError> {
        if self.no_local_socket.load(Ordering::SeqCst) {
            return Err(PlatformError::Unsupported("bind_local_socket"));
        }
        self.inner.bind_local_socket(path)
    }
    fn connect_local_socket(&self, path: &Path) -> Result<LocalStream, PlatformError> {
        self.inner.connect_local_socket(path)
    }
    fn current_uid(&self) -> Option<u32> {
        self.inner.current_uid()
    }
    fn watch_dirs(
        &self,
        dirs: &[PathBuf],
        changed: Box<dyn Fn() + Send>,
    ) -> Result<DirWatch, PlatformError> {
        self.inner.watch_dirs(dirs, changed)
    }
    fn device_name(&self) -> Option<String> {
        self.inner.device_name()
    }
    fn lan_ipv4_interfaces(&self) -> Result<Vec<LanInterface>, PlatformError> {
        self.inner.lan_ipv4_interfaces()
    }
    fn bind_broadcast_udp(&self, port: u16) -> io::Result<UdpSocket> {
        self.inner.bind_broadcast_udp(port)
    }
    fn bind_udp(&self, port: u16, buffer: usize) -> io::Result<(UdpSocket, SocketBuffers)> {
        self.inner.bind_udp(port, buffer)
    }
    fn clipboard_read(&self) -> Result<Option<ClipContent>, PlatformError> {
        self.inner.clipboard_read()
    }
    fn clipboard_write(&self, content: &ClipContent) -> Result<(), PlatformError> {
        self.inner.clipboard_write(content)
    }
    fn watch_clipboard(
        &self,
        on_change: Box<dyn Fn() + Send>,
    ) -> Result<ClipboardWatch, PlatformError> {
        self.inner.watch_clipboard(on_change)
    }
    fn downloads_dir(&self) -> Result<PathBuf, PlatformError> {
        self.inner.downloads_dir()
    }
    fn open_url(&self, url: &str) -> Result<(), PlatformError> {
        self.record(Done::OpenUrl(url.to_owned()))
    }
    fn open_path(&self, path: &Path) -> Result<(), PlatformError> {
        self.record(Done::OpenPath(path.to_owned()))
    }
    fn ring(&self, seconds: u16) -> Result<(), PlatformError> {
        self.record(Done::Ring(seconds))
    }
    fn stop_ring(&self) -> Result<(), PlatformError> {
        self.record(Done::StopRing)
    }
    fn link_keepalive(&self) -> Duration {
        self.inner.link_keepalive()
    }
}
