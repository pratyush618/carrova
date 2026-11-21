//! This app: version, device, and app updates.

use carrova_platform::OsKind;
use serde::Serialize;

use crate::update::{DownloadView, UpdateBlocker};

/// `about` result: what the About sheet shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct About {
    /// Version of the app (front-end build).
    pub app_version: String,
    /// Version of the engine ([`carrova_core::about::ENGINE_VERSION`]).
    pub engine_version: String,
    /// This machine's name, if the platform can tell
    /// ([`carrova_core::about::device_name`]).
    pub device_name: Option<String>,
    /// OS family.
    pub os: OsKind,
    /// The desktop shows status-tray icons; without one, closing the
    /// window minimises it instead of going to the tray.
    pub status_tray: bool,
}

/// An app update announced by the release feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct UpdateInfo {
    /// Version on offer.
    pub version: String,
    /// Version running now.
    pub current_version: String,
    /// Release notes (Markdown), if any.
    pub notes: Option<String>,
    /// Publication date as published in the feed (RFC 3339), if any.
    pub date: Option<String>,
}

/// Outcome of an app-updater command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UpdateStatus {
    /// The running version is the newest.
    UpToDate,
    /// A newer version can be installed.
    Available(UpdateInfo),
    /// The update is installed; it runs after a restart.
    Installed {
        /// Installed version.
        version: String,
    },
    /// A phone's checked update was handed to the OS installer, which
    /// asks the user; nothing is installed until they agree.
    Handed {
        /// Version handed over.
        version: String,
    },
}

/// Where an approved update install is now, between the user's go-ahead
/// and the `install_update` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum UpdateStep {
    /// The package is downloading.
    Downloading(DownloadView),
    /// The user paused the download (a phone's); it picks up from here.
    Paused(DownloadView),
    /// Downloaded; its signature is being checked against the release key.
    Verifying,
    /// Held until `blocker` ends, or the user says to go on anyway: a
    /// verified install while a move or call runs, a phone's download
    /// while it is off Wi-Fi.
    Waiting {
        /// What holds it.
        blocker: UpdateBlocker,
    },
    /// Downloaded and verified; the OS needs the user's leave to install
    /// apps first (a phone's "Install unknown apps").
    NeedsPermission,
    /// Installing now (a phone: handed to the OS installer, which asks).
    Installing,
}
