//! Operating-system identity ([`OsKind`]) and the permission model that
//! follows from it ([`PermissionModel`]).

use std::fmt;
use std::str::FromStr;

/// Operating-system family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum OsKind {
    /// Linux (any distribution).
    Linux,
    /// Apple macOS.
    MacOs,
    /// Microsoft Windows.
    Windows,
}

impl OsKind {
    /// Every OS family, in stable order.
    pub const ALL: [Self; 3] = [Self::Linux, Self::MacOs, Self::Windows];

    /// Stable lowercase identifier (`linux`, `macos`, `windows`), e.g. the
    /// source OS recorded in a manifest. [`FromStr`] parses it back. Not
    /// the serde form (`mac_os`), which the desktop IPC bindings use.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::MacOs => "macos",
            Self::Windows => "windows",
        }
    }

    /// Name shown to users (`Linux`, `macOS`, `Windows`).
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Linux => "Linux",
            Self::MacOs => "macOS",
            Self::Windows => "Windows",
        }
    }

    /// How this OS family protects files (see [`PermissionModel`]).
    #[must_use]
    pub const fn permission_model(self) -> PermissionModel {
        match self {
            Self::Linux | Self::MacOs => PermissionModel::Unix,
            Self::Windows => PermissionModel::Acl,
        }
    }
}

/// Renders [`OsKind::display_name`].
impl fmt::Display for OsKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.display_name())
    }
}

/// Parses the stable id produced by [`OsKind::as_str`] (exact match).
impl FromStr for OsKind {
    type Err = UnknownOsKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|os| os.as_str() == s)
            .ok_or_else(|| UnknownOsKind(s.to_owned()))
    }
}

/// A string that is not an [`OsKind::as_str`] id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown OS `{0}` (expected `linux`, `macos` or `windows`)")]
pub struct UnknownOsKind(pub String);

/// How an OS protects a user's files from other local users.
///
/// Callers that store, compare or enforce permission bits (e.g. "a resume
/// key must be `0600`") consult this instead of treating a missing
/// [`crate::FileMeta::mode`] as an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionModel {
    /// POSIX mode bits (`rwx` for owner/group/other) are authoritative:
    /// they are recorded on capture, restored exactly and checked on
    /// secrets.
    Unix,
    /// Access-control lists decide access (Windows). Mode bits are neither
    /// stored nor enforced: [`crate::FileMeta::mode`] is `None`, requested
    /// modes are ignored, and per-user privacy comes from the owner-only
    /// ACLs the OS puts on the user's profile directories.
    Acl,
}
