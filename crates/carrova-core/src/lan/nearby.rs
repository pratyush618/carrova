//! Which devices are nearby ([`browse_nearby`]): senders offering a code,
//! and presences, known ones recognised by their tags.

use std::cmp::Reverse;
use std::collections::HashSet;

use carrova_crypto::identity::DeviceId;
use carrova_crypto::pairing::KnownSecret;
use carrova_platform::{OsKind, Platform};
use carrova_transport::{Nearby, browse};

use super::{BROWSE_WINDOW, clean_device_name};
use crate::devices::{DEVICES_DIR, DeviceRecord, KnownDevices};
use crate::error::Result;
use crate::fsx;

/// A device found nearby.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NearbyDevice {
    /// Display name: a known device's local name from the store, else the
    /// name it advertised, sanitised.
    pub name: String,
    /// Its OS family: from the store if known, else as advertised.
    pub os: Option<OsKind>,
    /// The known device it is (its advert carried a tag only that device
    /// could make), never inferred from a name.
    pub known: Option<DeviceId>,
    /// Whether this is a sender offering a pairing code (only these belong
    /// in a "type the code" picker), else a presence. A known device that
    /// offers a code and is present shows once as each: a code offer
    /// carries no tag, so the two are never merged by name.
    pub offers_code: bool,
    /// Milliseconds after the browse started that it first answered.
    pub answered_ms: u32,
}

/// Devices on the LAN, collected for [`BROWSE_WINDOW`]: known devices by
/// their stored name, others by their advertised name sanitised like the
/// receiver's [`Inventory::device_name`](super::Inventory::device_name)
/// (names that sanitise to nothing, and unknown devices that give none,
/// are left out). The user still types the code to pair with a new one.
/// A known device is listed once, at its first answer, even if it
/// re-announced (a presence refresh) during the browse.
///
/// An unreadable known-device store is logged and the browse goes on
/// without recognising anyone.
///
/// # Errors
///
/// Discovery could not start.
pub fn browse_nearby(platform: &dyn Platform) -> Result<Vec<NearbyDevice>> {
    let found = browse(platform, BROWSE_WINDOW)?;
    let known = known_by_recent_use(platform, usize::MAX).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "known devices unreadable: none recognised nearby");
        Vec::new()
    });
    Ok(once_per_known_device(
        found.into_iter().filter_map(|n| nearby_device(n, &known)),
    ))
}

/// `found` in order, without a known device's later sightings.
fn once_per_known_device(found: impl IntoIterator<Item = NearbyDevice>) -> Vec<NearbyDevice> {
    let mut seen = HashSet::new();
    found
        .into_iter()
        .filter(|d| d.known.is_none_or(|id| seen.insert(id)))
        .collect()
}

/// Up to `limit` known devices that have a secret, most recently moved
/// with first (then most recently first seen); secrets past `limit` are not
/// read. Read-only: without a store there are none (it is never created
/// here).
pub(crate) fn known_by_recent_use(
    platform: &dyn Platform,
    limit: usize,
) -> Result<Vec<(DeviceRecord, KnownSecret)>> {
    if fsx::lstat(platform, &platform.state_dir()?.join(DEVICES_DIR))?.is_none() {
        return Ok(Vec::new());
    }
    let store = KnownDevices::open(platform)?;
    let mut records = store.list(platform)?;
    records.sort_by_key(|r| Reverse((r.last_move, r.first_seen)));
    let mut known = Vec::new();
    for record in records {
        if known.len() == limit {
            break;
        }
        if let Some(secret) = store.secret_for(platform, &record.public)? {
            known.push((record, secret));
        }
    }
    Ok(known)
}

/// `found` as shown: the known device whose secret made one of its tags,
/// else a stranger by its sanitised name.
fn nearby_device(found: Nearby, known: &[(DeviceRecord, KnownSecret)]) -> Option<NearbyDevice> {
    let answered_ms = u32::try_from(found.answered.as_millis()).unwrap_or(u32::MAX);
    let advert = found.advert;
    let record = advert.known.as_ref().and_then(|tags| {
        known
            .iter()
            .find(|(record, secret)| tags.made_with(secret, &record.public))
            .map(|(record, _)| record)
    });
    Some(match record {
        Some(record) => NearbyDevice {
            name: record.name.clone(),
            os: Some(record.os),
            known: Some(record.public),
            offers_code: found.offers_code,
            answered_ms,
        },
        None => NearbyDevice {
            name: clean_device_name(advert.name.as_deref()?)?,
            os: advert.os,
            known: None,
            offers_code: found.offers_code,
            answered_ms,
        },
    })
}

#[cfg(test)]
#[path = "../../tests/lan/nearby/unit.rs"]
mod tests;
