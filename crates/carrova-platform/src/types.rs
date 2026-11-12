use std::ffi::OsString;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::time::SystemTime;

/// Kind of filesystem object (as seen by lstat — symlinks are not followed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
    File,
    Dir,
    Symlink,
    Other,
}

/// One entry of a directory listing ([`crate::Platform::read_dir`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// File name within the directory (never `.` or `..`).
    pub name: OsString,
    /// Kind of the entry itself (lstat semantics: a symlink is reported as
    /// [`FileKind::Symlink`], never as its target's kind).
    pub kind: FileKind,
}

/// A filesystem as the user knows it ([`crate::Platform::disk`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskInfo {
    /// Capacity of the filesystem in bytes.
    pub total: u64,
    /// Where the filesystem is mounted (Windows: the volume root, e.g. `C:\`).
    pub mount_point: PathBuf,
}

/// Portable subset of file metadata Carrova cares about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    pub kind: FileKind,
    /// Size in bytes (for symlinks: length of the link target string).
    pub size: u64,
    /// Permission bits (e.g. `0o600`). `None` where the OS has no such concept.
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub modified: Option<SystemTime>,
}

/// A running process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    /// Short process name (Linux: `comm`).
    pub name: String,
    /// Executable path, when readable.
    pub exe: Option<PathBuf>,
}

impl ProcessInfo {
    /// Longest process name the kernel reports (Linux `comm`: 15 bytes + NUL).
    const TRUNCATED_NAME_LEN: usize = 15;

    /// Whether this process is an instance of executable `binary` (bare name).
    ///
    /// Matches the executable's file name, or the process name — including a
    /// name the OS truncated (Linux `comm` keeps 15 bytes), which matches when
    /// it prefixes `binary`.
    #[must_use]
    pub fn matches_binary(&self, binary: &str) -> bool {
        let exe_name = self
            .exe
            .as_deref()
            .and_then(std::path::Path::file_name)
            .and_then(|n| n.to_str());
        self.name == binary
            || exe_name == Some(binary)
            || (self.name.len() == Self::TRUNCATED_NAME_LEN
                && binary.starts_with(self.name.as_str()))
    }
}

/// Snapshot support of the filesystem holding a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapshotCapability {
    None,
    Btrfs,
    Lvm,
}

/// Opaque identity of a filesystem (Linux: the `st_dev` of a path). Two
/// paths with equal ids live on the same filesystem, so a rename between
/// them never crosses devices and they share free space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FsId(u64);

impl FsId {
    /// Wrap a raw device id (adapters and the fake platform only).
    #[must_use]
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

/// How [`crate::Platform::preallocate`] sized a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Allocation {
    /// Blocks are reserved: writes within the length cannot run out of space.
    Reserved,
    /// Only the length was set (the filesystem cannot reserve blocks):
    /// writes may still fail with `StorageFull`.
    Sparse,
}

/// A usable LAN IPv4 interface address (see
/// [`crate::Platform::lan_ipv4_interfaces`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanInterface {
    /// The interface's IPv4 address.
    pub ip: Ipv4Addr,
    /// Directed broadcast address of its subnet, if it has one.
    pub broadcast: Option<Ipv4Addr>,
}

/// Socket buffer sizes the OS granted (bytes; may be less than requested).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketBuffers {
    /// `SO_SNDBUF`.
    pub send: usize,
    /// `SO_RCVBUF`.
    pub recv: usize,
}

/// The "Send with Carrova" file-manager entries of this user
/// ([`crate::Platform::file_manager_status`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FileManagerStatus {
    /// This OS has file-manager entries Carrova can write.
    pub supported: bool,
    /// Carrova's entries are written.
    pub installed: bool,
    /// File managers left untouched and why, in the user's words (e.g.
    /// "Thunar: uca.xml is not valid; left unchanged").
    pub skipped: Vec<String>,
}

/// Whether the screen is locked, as far as the OS will say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScreenLock {
    /// The lock screen or screensaver is showing.
    Locked,
    /// The screen is in use.
    Unlocked,
    /// The OS cannot say (or did not answer). Callers must treat the
    /// screen as possibly locked.
    #[default]
    Unknown,
}
