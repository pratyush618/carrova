//! Carrova platform layer — the **only** OS-aware crate in the workspace.
//!
//! Everything above this crate talks to the operating system exclusively through
//! the [`Platform`] trait. Adding OS behaviour means adding a trait method here,
//! never a `cfg(target_os)` elsewhere.

mod device;
mod error;
mod lock;
mod meta;
pub mod path;
mod platform;
mod root;
mod types;

#[cfg(feature = "fake")]
pub mod fake;
#[cfg(target_os = "linux")]
mod fsops;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod netops;
#[cfg(target_os = "windows")]
mod windows;

pub use error::PlatformError;
pub use lock::FileLock;
pub use platform::Platform;
pub use root::SemanticRoot;
pub use types::{
    Allocation, DirEntry, FileKind, FileMeta, FsId, LanInterface, OsKind, ProcessInfo,
    SnapshotCapability, SocketBuffers,
};

/// Platform adapter for the OS this binary was compiled for.
#[must_use]
pub fn current() -> Box<dyn Platform> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxPlatform::from_env())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacPlatform)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsPlatform)
    }
}
