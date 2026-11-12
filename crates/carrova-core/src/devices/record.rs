//! One known device, as stored in `known.json`.

use std::fmt;

use carrova_crypto::identity::{DevicePublic, Fingerprint};
use carrova_platform::OsKind;
use serde::{Deserialize, Serialize};

/// A device this one has paired with and remembers. Identified by its
/// full public key; the name is a local, renamable label. `Debug` shows
/// the fingerprint only and never the MAC hint.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRecord {
    /// The device's identity key ([`DeviceId`](carrova_crypto::identity::DeviceId)).
    pub public: DevicePublic,
    /// Display name (set at pairing, renamable).
    pub name: String,
    /// The device's operating system.
    pub os: OsKind,
    /// When it was first remembered (unix seconds).
    pub first_seen: u64,
    /// When the last move with it finished (unix seconds).
    pub last_move: Option<u64>,
    /// Moves made with it.
    pub moves: u32,
    /// Opt-in Wake-on-LAN hint. Personal data: never logged.
    pub mac_hint: Option<[u8; 6]>,
}

impl DeviceRecord {
    /// Display label of the device's key.
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.public)
    }
}

impl fmt::Debug for DeviceRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceRecord")
            .field("fingerprint", &self.fingerprint())
            .field("name", &self.name)
            .field("os", &self.os)
            .field("first_seen", &self.first_seen)
            .field("last_move", &self.last_move)
            .field("moves", &self.moves)
            .finish_non_exhaustive()
    }
}

/// How a remembered move changes a device's saved wake hint.
#[derive(Clone, Copy)]
pub enum HintUpdate {
    /// Hints are on but none was found: keep what is saved.
    Keep,
    /// Save this hardware address.
    Set([u8; 6]),
    /// Hints are off: stop keeping the saved address.
    Clear,
}

impl fmt::Debug for HintUpdate {
    /// Never shows the address.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Keep => "Keep",
            Self::Set(_) => "Set(..)",
            Self::Clear => "Clear",
        })
    }
}

impl HintUpdate {
    /// The hint to save given the one saved now.
    pub(super) fn apply(self, saved: Option<[u8; 6]>) -> Option<[u8; 6]> {
        match self {
            Self::Keep => saved,
            Self::Set(mac) => Some(mac),
            Self::Clear => None,
        }
    }
}
