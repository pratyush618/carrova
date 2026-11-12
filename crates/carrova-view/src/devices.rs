//! Nearby and known devices, and who may find this computer: the views the
//! Nearby page, the known-devices list, the tray and the CLI show. Pure (no
//! I/O, no escaping): device names are peer-provided or user-typed, and
//! front-ends escape them.
//!
//! A device's `id` is its full public key in hex: the handle commands take
//! back, never shown or logged. Its fingerprint is the display label.

use carrova_core::devices::DeviceRecord;
use carrova_core::lan::NearbyDevice;
use carrova_core::presence::Visibility;
use carrova_crypto::identity::Fingerprint;
use carrova_platform::OsKind;
use serde::{Deserialize, Serialize};

/// Devices that answered a browse within this many milliseconds sit on
/// the map's inner ring ("answering now").
pub const INNER_RING_MS: u32 = 300;

/// Which ring of the Nearby map a device sits on. Not distance: how fast
/// it answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Ring {
    /// Answered within [`INNER_RING_MS`].
    Inner,
    /// Answered later.
    Outer,
}

/// The ring of a device that answered `answered_ms` after the browse began.
#[must_use]
pub const fn ring(answered_ms: u32) -> Ring {
    if answered_ms < INNER_RING_MS {
        Ring::Inner
    } else {
        Ring::Outer
    }
}

/// A device found nearby, as the Nearby page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NearbyDeviceView {
    /// Its name: a known device's local name, else as it announced it.
    pub name: String,
    /// Its OS family, if known.
    pub os: Option<OsKind>,
    /// A device this one remembers (it proved so by its tag).
    pub known: bool,
    /// A known device's id, for commands; `None` for a new one.
    pub id: Option<String>,
    /// A known device's fingerprint (`7F3A·19C2`).
    pub fingerprint: Option<String>,
    /// Milliseconds after the browse began that it answered.
    pub answered_ms: u32,
    /// Where it sits on the map ([`ring`]).
    pub ring: Ring,
    /// A sender offering a pairing code (else a presence).
    pub offers_code: bool,
}

/// `found` as the Nearby page lists it, in the order found.
#[must_use]
pub fn nearby_views(found: &[NearbyDevice]) -> Vec<NearbyDeviceView> {
    found
        .iter()
        .map(|d| NearbyDeviceView {
            name: d.name.clone(),
            os: d.os,
            known: d.known.is_some(),
            id: d.known.map(|id| id.to_hex()),
            fingerprint: d.known.map(|id| Fingerprint::of(&id).to_string()),
            answered_ms: d.answered_ms,
            ring: ring(d.answered_ms),
            offers_code: d.offers_code,
        })
        .collect()
}

/// A remembered device, as Settings → Known devices lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KnownDeviceView {
    /// Its id, for commands.
    pub id: String,
    /// Its fingerprint (`7F3A·19C2`).
    pub fingerprint: String,
    /// Its local (renamable) name.
    pub name: String,
    /// Its OS family.
    pub os: OsKind,
    /// When it was first remembered (Unix seconds).
    pub first_seen: u64,
    /// When the last move with it finished (Unix seconds).
    pub last_move: Option<u64>,
    /// Moves made with it.
    pub moves: u32,
    /// A Wake-on-LAN hint is kept (the hint itself never leaves the engine).
    pub has_wake_hint: bool,
}

/// `records` as the known-devices list shows them, in stored order.
#[must_use]
pub fn known_device_views(records: &[DeviceRecord]) -> Vec<KnownDeviceView> {
    records
        .iter()
        .map(|r| KnownDeviceView {
            id: r.public.to_hex(),
            fingerprint: r.fingerprint().to_string(),
            name: r.name.clone(),
            os: r.os,
            first_seen: r.first_seen,
            last_move: r.last_move,
            moves: r.moves,
            has_wake_hint: r.mac_hint.is_some(),
        })
        .collect()
}

/// Who may find this computer nearby, as the user picks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum VisibilityChoice {
    /// Nobody.
    NoOne,
    /// Known devices only.
    Known,
    /// Everyone, for
    /// [`EVERYONE_FOR`](carrova_core::presence::EVERYONE_FOR); then known
    /// devices only.
    Everyone,
}

/// The visibility in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VisibilityView {
    /// Who may find this computer.
    pub choice: VisibilityChoice,
    /// When "everyone" ends (Unix seconds); `None` otherwise.
    pub until: Option<u64>,
}

/// The view of `visibility` (pass the one in force:
/// [`Visibility::at`]).
#[must_use]
pub const fn visibility_view(visibility: Visibility) -> VisibilityView {
    match visibility {
        Visibility::NoOne => VisibilityView {
            choice: VisibilityChoice::NoOne,
            until: None,
        },
        Visibility::Known => VisibilityView {
            choice: VisibilityChoice::Known,
            until: None,
        },
        Visibility::Everyone { until } => VisibilityView {
            choice: VisibilityChoice::Everyone,
            until: Some(until),
        },
    }
}

/// The visibility `choice` means when picked at `now` (Unix seconds):
/// "everyone" runs from now ([`Visibility::everyone_from`]).
#[must_use]
pub const fn chosen_visibility(choice: VisibilityChoice, now: u64) -> Visibility {
    match choice {
        VisibilityChoice::NoOne => Visibility::NoOne,
        VisibilityChoice::Known => Visibility::Known,
        VisibilityChoice::Everyone => Visibility::everyone_from(now),
    }
}

/// Which known devices to forget.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ForgetDevices {
    /// Every known device.
    All,
    /// The one device with this id or fingerprint.
    Device(String),
}
