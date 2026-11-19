//! A sideloaded app updating itself (Android): the facts of an app
//! package, and the download as the OS shows it.

use serde::Deserialize;

/// An app package as the OS reads it, from an APK file or the installed
/// app ([`crate::Platform::inspect_package`],
/// [`crate::Platform::installed_package`]).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppPackage {
    /// Application id (`org.byteveda.Carrova`).
    pub package: String,
    /// Build number; a newer build has a larger one.
    pub version_code: u64,
    /// SHA-256 of each certificate the package's contents are signed with.
    pub signers: Vec<[u8; 32]>,
}

/// What an app update download is doing, as its notification says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStage {
    /// Bytes are arriving.
    Downloading,
    /// The user paused it.
    Paused,
    /// It waits for Wi-Fi (no download on mobile data).
    WaitingForWifi,
}

/// The ongoing notification of an app update download
/// ([`crate::Platform::show_update`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateNotice {
    /// Version being downloaded.
    pub version: String,
    /// What the download is doing.
    pub stage: UpdateStage,
    /// Bytes downloaded so far.
    pub done: u64,
    /// Size of the package, if known.
    pub total: Option<u64>,
    /// Measured speed, once there is one.
    pub bytes_per_sec: Option<u64>,
}

/// A tap on the update notification's actions
/// ([`crate::Platform::take_update_tap`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTap {
    /// Pause the download.
    Pause,
    /// Carry on with a paused download.
    Resume,
    /// Stop the update; nothing is installed.
    Cancel,
}
