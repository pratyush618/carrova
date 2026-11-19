//! Carrova platform layer — the **only** OS-aware crate in the workspace.
//!
//! Everything above this crate talks to the operating system exclusively through
//! the [`Platform`] trait. Adding OS behaviour means adding a trait method here,
//! never a `cfg(target_os)` elsewhere.

mod app_update;
mod clipboard;
mod device;
mod download;
mod error;
mod local_socket;
mod lock;
mod meta;
#[cfg(any(test, target_os = "linux", target_os = "macos", target_os = "windows"))]
mod neighbours;
#[cfg(any(test, target_os = "macos", target_os = "windows"))]
mod notify_watch;
#[cfg(any(test, target_os = "linux", target_os = "android"))]
mod open;
mod os;
pub mod path;
pub mod phone;
mod platform;
mod root;
#[cfg(any(test, target_os = "linux", target_os = "macos", target_os = "windows"))]
mod tool;
mod types;
mod watch;

#[cfg(any(test, target_os = "android"))]
mod android;
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

#[cfg(target_os = "android")]
pub use android::attach_service_context;
pub use app_update::{AppPackage, UpdateNotice, UpdateStage, UpdateTap};
pub use clipboard::{ClipContent, ClipKind, ClipboardWatch};
pub use download::DownloadSink;
pub use error::PlatformError;
pub use local_socket::{LocalListener, LocalStream};
pub use lock::{FileLock, MulticastGuard};
pub use os::{OsKind, PermissionModel, UnknownOsKind};
pub use platform::Platform;
pub use root::SemanticRoot;
pub use types::{
    Allocation, DirEntry, DiskInfo, FileKind, FileManagerStatus, FileMeta, FsId, LanInterface,
    LinkNotice, ProcessInfo, ScreenLock, SnapshotCapability, SocketBuffers,
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
    #[cfg(target_os = "android")]
    {
        Box::new(android::AndroidPlatform::from_env())
    }
}
