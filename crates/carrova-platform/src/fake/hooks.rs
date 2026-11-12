//! Fault injection for [`FakePlatform`](super::FakePlatform): tests make one
//! operation fail, race or misreport without wrapping the whole trait.

use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::{FileMeta, PlatformError, ScreenLock};

/// Rewrites what [`Platform::metadata`](crate::Platform::metadata) (lstat)
/// reports for a path.
pub type MetadataHook = Arc<dyn Fn(&Path, &mut FileMeta) + Send + Sync>;
/// Runs before [`Platform::rename_noreplace`](crate::Platform::rename_noreplace)
/// `(from, to)`; an `Err` is returned instead of renaming.
pub type RenameHook = Arc<dyn Fn(&Path, &Path) -> Result<(), PlatformError> + Send + Sync>;
/// Runs right after [`Platform::open_read_nofollow`](crate::Platform::open_read_nofollow)
/// opened a path (e.g. to swap the file under the open fd).
pub type OpenHook = Arc<dyn Fn(&Path) + Send + Sync>;
/// Runs after [`Platform::file_manager_status`](crate::Platform::file_manager_status)
/// read the entries, before it returns (e.g. to stall a reader mid-race).
pub type StatusHook = Arc<dyn Fn() + Send + Sync>;

/// Faults to inject; every field defaults to "behave normally".
#[derive(Clone, Default)]
pub struct Hooks {
    /// See [`MetadataHook`]. Applied after [`Hooks::hide_mode`].
    pub on_metadata: Option<MetadataHook>,
    /// See [`RenameHook`].
    pub on_rename: Option<RenameHook>,
    /// See [`OpenHook`].
    pub on_open: Option<OpenHook>,
    /// See [`StatusHook`].
    pub on_file_manager_status: Option<StatusHook>,
    /// Report this effective uid instead of the real one.
    pub uid: Option<u32>,
    /// Report no permission bits in path and fd metadata (an OS without
    /// modes).
    pub hide_mode: bool,
    /// The hardware address [`Platform::neighbour_mac`](crate::Platform::neighbour_mac)
    /// reports for every host (default: none).
    pub neighbour_mac: Option<[u8; 6]>,
    /// Refuse [`Platform::watch_dirs`](crate::Platform::watch_dirs) as
    /// unsupported, as on macOS and Windows.
    pub unwatched: bool,
}

impl fmt::Debug for Hooks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hooks")
            .field("on_metadata", &self.on_metadata.is_some())
            .field("on_rename", &self.on_rename.is_some())
            .field("on_open", &self.on_open.is_some())
            .field(
                "on_file_manager_status",
                &self.on_file_manager_status.is_some(),
            )
            .field("uid", &self.uid)
            .field("hide_mode", &self.hide_mode)
            .field("neighbour_mac", &self.neighbour_mac.is_some())
            .field("unwatched", &self.unwatched)
            .finish()
    }
}

/// Hooks plus the knobs and counters a test changes or reads while the
/// platform is in use. Shared by clones of one hooked platform.
#[derive(Debug, Default)]
pub(super) struct HookState {
    pub(super) hooks: Hooks,
    free_space: Mutex<Option<u64>>,
    dir_syncs: AtomicUsize,
    wakes: AtomicUsize,
    magic_packets: Mutex<Vec<[u8; 6]>>,
    screen_lock: Mutex<ScreenLock>,
}

impl HookState {
    pub(super) fn new(hooks: Hooks) -> Self {
        Self {
            hooks,
            ..Self::default()
        }
    }

    pub(super) fn free_space(&self) -> Option<u64> {
        *self
            .free_space
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn set_free_space(&self, bytes: Option<u64>) {
        *self
            .free_space
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = bytes;
    }

    pub(super) fn count_dir_sync(&self) {
        self.dir_syncs.fetch_add(1, Ordering::SeqCst);
    }

    pub(super) fn dir_syncs(&self) -> usize {
        self.dir_syncs.load(Ordering::SeqCst)
    }

    pub(super) fn count_wake(&self) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }

    pub(super) fn wakes(&self) -> usize {
        self.wakes.load(Ordering::SeqCst)
    }

    pub(super) fn record_magic_packet(&self, mac: [u8; 6]) {
        self.magic_packets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(mac);
    }

    pub(super) fn magic_packets(&self) -> Vec<[u8; 6]> {
        self.magic_packets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(super) fn screen_lock(&self) -> ScreenLock {
        *self
            .screen_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn set_screen_lock(&self, lock: ScreenLock) {
        *self
            .screen_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = lock;
    }

    /// Path metadata as the hooks want it reported.
    pub(super) fn path_meta(&self, path: &Path, meta: FileMeta) -> FileMeta {
        let mut meta = self.fd_meta(meta);
        if let Some(hook) = &self.hooks.on_metadata {
            hook(path, &mut meta);
        }
        meta
    }

    /// Fd metadata as the hooks want it reported.
    pub(super) fn fd_meta(&self, mut meta: FileMeta) -> FileMeta {
        if self.hooks.hide_mode {
            meta.mode = None;
        }
        meta
    }
}
