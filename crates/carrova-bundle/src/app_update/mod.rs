//! A sideloaded phone's own app update. No store: the newest release's
//! APK comes from GitHub releases ([`FEED_URL`], [`Release::apk_url`]),
//! downloads resumably ([`download`]) and is handed to the OS installer
//! only when it is the same app, signed with Carrova's one release key
//! ([`RELEASE_CERT_SHA256`]) and newer than the build installed
//! ([`check_apk`]), checked again right before the hand-over
//! ([`install`]). The OS installer still asks the user.

mod control;
mod download;
mod error;
mod http;
mod install;
mod phone;
mod release;

pub use control::{Control, Running, Wish};
pub use download::{Job, download};
pub use error::{AppUpdateError, NetFailure};
pub use http::{Get, Https, Reply, Resume, fetch_release, trusted_url};
pub use install::{Progress, Step, install};
pub use phone::Phone;
pub use release::{
    ApkRejected, AppVersion, BadVersion, FeedError, Release, check_apk, just_updated, may_download,
};

/// SHA-256 of the certificate every release APK is signed with
/// (`apksigner verify --print-certs`). The one source of truth: the
/// release workflow checks the signed APK against this value.
pub const RELEASE_CERT_SHA256: [u8; 32] = [
    0x76, 0x05, 0xDF, 0xDE, 0xC3, 0x1E, 0x4B, 0x59, 0x51, 0xA6, 0x13, 0xD8, 0x4C, 0x28, 0xD1, 0xF4,
    0xAE, 0xB5, 0xEE, 0xEB, 0xD5, 0x37, 0x34, 0x50, 0x34, 0xFB, 0x86, 0xC8, 0xA3, 0xC1, 0xA3, 0xCB,
];

/// The newest release's feed, the file the desktop updater reads too
/// (`tauri.conf.json` → `plugins.updater.endpoints`).
pub const FEED_URL: &str = crate::fetch::releases_url!("latest/download/latest.json");

/// Largest feed read; a real one is a few kilobytes.
pub const FEED_MAX_SIZE: u64 = 256 * 1024;

/// Largest APK downloaded (a release is about 50 MB).
pub const APK_MAX_SIZE: u64 = 300 * 1024 * 1024;
