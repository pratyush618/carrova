//! Carrova platform layer — the **only** OS-aware crate in the workspace.
//!
//! Everything above this crate talks to the operating system exclusively through
//! the [`Platform`] trait. Adding OS behaviour means adding a trait method here,
//! never a `cfg(target_os)` elsewhere.

mod device;
mod error;
mod lock;
mod meta;
mod neighbours;
#[cfg(any(target_os = "macos", windows))]
mod notify_watch;
mod os;
pub mod path;
mod platform;
mod root;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod tool;
mod types;
mod watch;

#[cfg(feature = "fake")]
pub mod fake;
#[cfg(unix)]
mod fsops;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(unix, windows))]
mod netops;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(windows)]
use windows::fs as sys;

/// Filesystem primitives of this OS family, one signature set for all.
#[cfg(unix)]
use fsops as sys;

pub use error::PlatformError;
pub use lock::FileLock;
pub use os::{OsKind, PermissionModel, UnknownOsKind};
pub use platform::Platform;
pub use root::SemanticRoot;
pub use types::{
    Allocation, DirEntry, DiskInfo, FileKind, FileManagerStatus, FileMeta, FsId, LanInterface,
    ProcessInfo, ScreenLock, SnapshotCapability, SocketBuffers,
};
pub use watch::DirWatch;

/// Platform adapter for the OS this binary was compiled for.
#[must_use]
pub fn current() -> Box<dyn Platform> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxPlatform::from_env())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacPlatform::from_env())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsPlatform)
    }
}
