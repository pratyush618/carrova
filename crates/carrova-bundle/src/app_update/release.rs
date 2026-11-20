//! The release on offer, its APK, and the rules for downloading and
//! trusting it.

use std::fmt;
use std::str::FromStr;

use carrova_platform::AppPackage;
use serde::Deserialize;

use super::RELEASE_CERT_SHA256;
use crate::DEFAULT_BASE_URL;

/// A release version, `major.minor.patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

/// A version is not `major.minor.patch`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a release version")]
pub struct BadVersion(String);

impl FromStr for AppVersion {
    type Err = BadVersion;

    /// `1.2.3`, or `v1.2.3` as tags write it.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || BadVersion(s.to_owned());
        let mut parts = s.strip_prefix('v').unwrap_or(s).split('.');
        let mut next = || -> Result<u64, BadVersion> {
            let part = parts.next().ok_or_else(bad)?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return Err(bad());
            }
            part.parse().map_err(|_| bad())
        };
        let version = Self {
            major: next()?,
            minor: next()?,
            patch: next()?,
        };
        if parts.next().is_some() {
            return Err(bad());
        }
        Ok(version)
    }
}

impl fmt::Display for AppVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The feed cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum FeedError {
    /// Not the feed's JSON.
    #[error("the release feed is not valid")]
    Parse(#[from] serde_json::Error),
    /// Its version is not a release version.
    #[error(transparent)]
    Version(#[from] BadVersion),
}

/// The newest release as the feed describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Its version.
    pub version: AppVersion,
    /// Release notes (Markdown), if any.
    pub notes: Option<String>,
    /// Publication date (RFC 3339), if any.
    pub date: Option<String>,
}

/// The feed's fields a phone reads (the desktop's `platforms` are not).
#[derive(Deserialize)]
struct Feed {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
}

impl Release {
    /// The release described by the feed's `json`.
    ///
    /// # Errors
    ///
    /// [`FeedError`] if `json` is not the feed or names no release version.
    pub fn parse(json: &[u8]) -> Result<Self, FeedError> {
        let feed: Feed = serde_json::from_slice(json)?;
        Ok(Self {
            version: feed.version.parse()?,
            notes: feed.notes.filter(|notes| !notes.trim().is_empty()),
            date: feed.pub_date,
        })
    }

    /// The release's APK file name.
    // shortcut: arm64 only, upgrade when other ABIs ship.
    #[must_use]
    pub fn apk_name(&self) -> String {
        format!("Carrova_{}_android-arm64.apk", self.version)
    }

    /// Where the release's APK downloads from.
    #[must_use]
    pub fn apk_url(&self) -> String {
        format!("{DEFAULT_BASE_URL}v{}/{}", self.version, self.apk_name())
    }
}

/// Whether a phone may download now: on Wi-Fi, or anywhere when the user
/// allows mobile data (in Settings, or for this download).
#[must_use]
pub fn may_download(on_wifi: bool, mobile_data: bool) -> bool {
    on_wifi || mobile_data
}

/// Why a downloaded APK is not handed to the installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ApkRejected {
    /// It is another app.
    #[error("the download is not Carrova")]
    OtherApp,
    /// It is not signed with Carrova's release key (only).
    #[error("the download is not signed with Carrova's release key")]
    WrongSigner,
    /// It is no newer than the app installed.
    #[error("the download is not newer than the installed Carrova")]
    NotNewer,
}

/// Whether `apk` may update `installed`: the same app, signed with the
/// release key alone ([`RELEASE_CERT_SHA256`]), and a newer build.
///
/// # Errors
///
/// [`ApkRejected`] naming the first check it fails.
pub fn check_apk(apk: &AppPackage, installed: &AppPackage) -> Result<(), ApkRejected> {
    if apk.package != installed.package {
        return Err(ApkRejected::OtherApp);
    }
    if apk.signers != [RELEASE_CERT_SHA256] {
        return Err(ApkRejected::WrongSigner);
    }
    if apk.version_code <= installed.version_code {
        return Err(ApkRejected::NotNewer);
    }
    Ok(())
}

/// Whether the app was updated since it last ran: an earlier run recorded
/// another version (a first run announces nothing).
#[must_use]
pub fn just_updated(last_run: Option<&str>, running: &str) -> bool {
    last_run.is_some_and(|last| last != running)
}
