//! Test double: maps semantic roots into a directory tree you control
//! (e.g. a recipe fixture's fake home). Enabled with the `fake` feature.
//!
//! Filesystem writes (Phase 2 methods) and LAN sockets/interfaces (Phase 3)
//! are real and shared with the native adapter of the build OS (Unix or
//! Windows).
//!
//! Faults (failing renames, racing swaps, changing metadata, another uid)
//! are injected with [`FakePlatform::with_hooks`]. File-manager entries
//! are unsupported unless [`FakePlatform::file_manager_entries`] pretends
//! them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(doc)]
use crate::Platform;
use crate::{FsId, OsKind, ProcessInfo, SemanticRoot};

mod file_manager;
mod fixture;
mod hooks;
mod platform;
#[cfg(test)]
#[path = "../../tests/fake/unit.rs"]
mod tests;

pub use file_manager::EntriesWrite;
pub use fixture::symlink;
#[cfg(unix)]
pub use fixture::{pin_umask, write_tool_script};
pub use hooks::{Hooks, MetadataHook, OpenHook, RenameHook, StatusHook};

use file_manager::FakeEntries;
use hooks::HookState;

/// Deterministic [`Platform`] for tests. Filesystem metadata and operations
/// are real (point roots at a tempdir); roots, state dir, binaries, processes
/// and free space are configured.
#[derive(Debug, Clone)]
pub struct FakePlatform {
    /// OS family reported by [`Platform::os`] (default Linux).
    os: OsKind,
    roots: HashMap<SemanticRoot, PathBuf>,
    binaries: HashMap<String, PathBuf>,
    processes: Vec<ProcessInfo>,
    free_space: u64,
    /// Capacity reported by [`Platform::disk`].
    disk_total: u64,
    state_dir: PathBuf,
    /// Raw device name, sanitised like a native one when read.
    device_name: Option<String>,
    /// Fake filesystems: everything under the path is on that filesystem.
    filesystems: Vec<FakeFs>,
    /// Injected faults and runtime knobs (shared by clones).
    hooks: Arc<HookState>,
    /// Pretend file-manager entries (shared by clones); `None`: unsupported.
    entries: Option<Arc<FakeEntries>>,
}

/// A pretend filesystem mounted at `mount` (see [`FakePlatform::filesystem`]).
#[derive(Debug, Clone)]
struct FakeFs {
    mount: PathBuf,
    id: FsId,
    free: u64,
}

/// High bit marks fake filesystem ids so they never equal a real `st_dev`.
const FAKE_FS_BIT: u64 = 1 << 63;

impl FakePlatform {
    /// Roots laid out Linux-style under `home`:
    /// `.config`, `.local/share`, `.cache`, `.` and `Documents`; state dir
    /// `.local/state/carrova`.
    #[must_use]
    pub fn with_home(home: impl AsRef<Path>) -> Self {
        let home = home.as_ref();
        let roots = HashMap::from([
            (SemanticRoot::UserConfig, home.join(".config")),
            (SemanticRoot::UserData, home.join(".local/share")),
            (SemanticRoot::Cache, home.join(".cache")),
            (SemanticRoot::Home, home.to_path_buf()),
            (SemanticRoot::Documents, home.join("Documents")),
        ]);
        Self {
            os: OsKind::Linux,
            roots,
            binaries: HashMap::new(),
            processes: Vec::new(),
            free_space: u64::MAX,
            disk_total: u64::MAX,
            state_dir: home.join(".local/state/carrova"),
            device_name: None,
            filesystems: Vec::new(),
            hooks: Arc::default(),
            entries: None,
        }
    }

    /// A `os` machine whose roots sit under `home` where that OS puts
    /// them: Linux as [`Self::with_home`]; macOS `Library/Application
    /// Support` (config and data) and `Library/Caches`; Windows
    /// `AppData/Roaming` (config) and `AppData/Local` (data and cache).
    #[must_use]
    pub fn with_os_layout(home: impl AsRef<Path>, os: OsKind) -> Self {
        let home = home.as_ref();
        let p = Self::with_home(home).with_os(os);
        let (config, data, cache) = match os {
            OsKind::Linux => return p,
            OsKind::MacOs => (
                "Library/Application Support",
                "Library/Application Support",
                "Library/Caches",
            ),
            OsKind::Windows => ("AppData/Roaming", "AppData/Local", "AppData/Local"),
        };
        p.root(SemanticRoot::UserConfig, home.join(config))
            .root(SemanticRoot::UserData, home.join(data))
            .root(SemanticRoot::Cache, home.join(cache))
    }

    /// Pretend to be a `os` machine (default [`OsKind::Linux`]), e.g. a
    /// macOS or Windows peer in a cross-OS test. Only the identity changes:
    /// [`Platform::os`] and [`Platform::permission_model`] follow `os`
    /// (the latter only on a Unix host), while roots and filesystem
    /// operations stay as configured (named `with_` so it does not shadow
    /// the trait method).
    #[must_use]
    pub fn with_os(mut self, os: OsKind) -> Self {
        self.os = os;
        self
    }

    /// Override a single root.
    #[must_use]
    pub fn root(mut self, root: SemanticRoot, path: impl Into<PathBuf>) -> Self {
        self.roots.insert(root, path.into());
        self
    }

    /// Override the state directory returned by [`Platform::state_dir`]
    /// (named `with_` so it does not shadow that trait method).
    #[must_use]
    pub fn with_state_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_dir = path.into();
        self
    }

    /// Pretend the OS names this computer `raw`; [`Platform::device_name`]
    /// returns it sanitised like a native name (default: no name). Named
    /// `with_` so it does not shadow that trait method.
    #[must_use]
    pub fn with_device_name(mut self, raw: impl Into<String>) -> Self {
        self.device_name = Some(raw.into());
        self
    }

    /// Pretend `name` is installed at `path`.
    #[must_use]
    pub fn binary(mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        self.binaries.insert(name.into(), path.into());
        self
    }

    /// Pretend a process is running.
    #[must_use]
    pub fn process(mut self, pid: u32, name: impl Into<String>) -> Self {
        self.processes.push(ProcessInfo {
            pid,
            name: name.into(),
            exe: None,
        });
        self
    }

    /// Set reported free space (for paths outside every
    /// [`Self::filesystem`]). It is also the allocation limit:
    /// [`Platform::preallocate`] of more bytes fails with
    /// `io::ErrorKind::StorageFull`, like a full disk.
    #[must_use]
    pub fn free_space_bytes(mut self, bytes: u64) -> Self {
        self.free_space = bytes;
        self
    }

    /// Set the capacity [`Platform::disk`] reports for every path
    /// (default `u64::MAX`).
    #[must_use]
    pub fn disk_total_bytes(mut self, bytes: u64) -> Self {
        self.disk_total = bytes;
        self
    }

    /// Pretend everything under `mount` is a separate filesystem number
    /// `id` with `free` bytes available ([`Platform::fs_id`] and
    /// [`Platform::free_space`]; the longest matching mount wins). Paths
    /// outside every fake filesystem report their real filesystem.
    #[must_use]
    pub fn filesystem(mut self, mount: impl Into<PathBuf>, id: u32, free: u64) -> Self {
        self.filesystems.push(FakeFs {
            mount: mount.into(),
            id: FsId::new(FAKE_FS_BIT | u64::from(id)),
            free,
        });
        self
    }

    /// Inject `hooks`. Starts a fresh hook state (counters at zero, no
    /// free-space override) that later clones of the result share; the
    /// platform this was called on is unaffected.
    #[must_use]
    pub fn with_hooks(mut self, hooks: Hooks) -> Self {
        self.hooks = Arc::new(HookState::new(hooks));
        self
    }

    /// While `Some`, every [`Platform::free_space`] query reports this many
    /// bytes, whatever the path (changeable while the platform is in use;
    /// allocation limits are unaffected).
    pub fn override_free_space(&self, bytes: Option<u64>) {
        self.hooks.set_free_space(bytes);
    }

    /// [`Platform::sync_dir`] calls since [`Self::with_hooks`].
    #[must_use]
    pub fn dir_syncs(&self) -> usize {
        self.hooks.dir_syncs()
    }

    /// [`Platform::wake_display`] calls since [`Self::with_hooks`].
    #[must_use]
    pub fn wakes(&self) -> usize {
        self.hooks.wakes()
    }

    /// Make [`Platform::screen_locked`] report `locked` (changeable while
    /// the platform is in use).
    pub fn set_screen_locked(&self, locked: bool) {
        self.hooks.set_screen_locked(locked);
    }

    /// Pretend this OS has file-manager entries, `installed` or not at
    /// start. Installs and removes write nothing; they are recorded
    /// ([`Self::entries_writes`]) and decide what the status reports.
    #[must_use]
    pub fn file_manager_entries(mut self, installed: bool) -> Self {
        self.entries = Some(Arc::new(FakeEntries::new(installed)));
        self
    }

    /// Every install and remove of the file-manager entries, in call order.
    #[must_use]
    pub fn entries_writes(&self) -> Vec<EntriesWrite> {
        self.entries
            .as_ref()
            .map(|e| e.writes())
            .unwrap_or_default()
    }

    fn fake_fs(&self, path: &Path) -> Option<&FakeFs> {
        self.filesystems
            .iter()
            .filter(|f| path.starts_with(&f.mount))
            .max_by_key(|f| f.mount.components().count())
    }
}
