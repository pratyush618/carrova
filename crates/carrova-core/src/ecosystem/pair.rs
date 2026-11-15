//! Linking two devices without moving anything (ADR 0002 §1–2): one side
//! offers a code, the other joins with the scanned invite (key pinned) or
//! the typed code (unpinned). The offering side's user checks the device
//! found before anything is stored. Both remember the other and start it
//! on its default policy.

use std::sync::Arc;
use std::time::{Duration, Instant};

use carrova_crypto::identity::{DevicePublic, Fingerprint};
use carrova_crypto::pairing::KnownSecret;
use carrova_link::PolicyDoc;
use carrova_platform::{OsKind, Platform};
use carrova_transport::{
    ConnectOptions, ListenOptions, Listener, PAIR_CONFIRM_TIMEOUT, PairedPeer, PeerInfo, Ticket,
    pair_only,
};

use super::{PairInvite, PolicyStore};
use crate::cancel::Cancel;
use crate::devices::{DeviceRecord, KnownDevices};
use crate::engine::Engine;
use crate::engine::offer;
use crate::error::{CoreError, Result};
use crate::lan::{CODE_LIFETIME, PAIR_TIMEOUT, clean_device_name};
use crate::presence::announced_name;
use crate::time;

/// How long the user has to confirm a found device: the joiner's wait
/// ([`PAIR_CONFIRM_TIMEOUT`]) less a margin for confirming over the wire.
pub const PAIR_CONFIRM_WINDOW: Duration =
    PAIR_CONFIRM_TIMEOUT.saturating_sub(Duration::from_secs(10));

/// The device that answered a pairing offer, shown for the user's key
/// check before it is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerFound {
    name: String,
    os: OsKind,
    public: DevicePublic,
}

impl PeerFound {
    /// The device called `name` (cleaned for display; its fingerprint if
    /// nothing visible is left) on `os`, proven to hold `public`.
    #[must_use]
    pub fn new(name: &str, os: OsKind, public: DevicePublic) -> Self {
        Self {
            name: display_name(name, &public),
            os,
            public,
        }
    }

    /// Its cleaned name (untrusted: for display only).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its OS family (untrusted: for display only).
    #[must_use]
    pub fn os(&self) -> OsKind {
        self.os
    }

    /// Its proven device key.
    #[must_use]
    pub fn public(&self) -> DevicePublic {
        self.public
    }

    /// Its key's fingerprint, to compare with the one the device shows.
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.public)
    }
}

/// Wait for a device to link: show `on_code` the invite (QR and typed
/// code), then pair with whoever answers it and ask `on_peer` whether it
/// is the device the user meant. Remembers it, even when "remember
/// devices" is off (linking is an explicit choice), only on a yes given
/// within [`PAIR_CONFIRM_WINDOW`]; a no or a late answer fails with
/// [`CoreError::PairRejected`] and nothing is stored on either side.
///
/// Blocks up to [`CODE_LIFETIME`] plus the answer, or until `cancel`
/// fires ([`CoreError::Cancelled`](crate::CoreError::Cancelled)).
pub fn offer_pairing(
    engine: &Engine,
    cancel: &Cancel,
    on_code: impl FnOnce(PairInvite),
    on_peer: impl FnOnce(&PeerFound) -> bool,
) -> Result<DeviceRecord> {
    let platform = &**engine.platform();
    let key = Arc::new(KnownDevices::open(platform)?.device_key(platform)?);
    let mut listener = Listener::bind(ListenOptions {
        platform: Arc::clone(engine.platform()),
        device_name: Some(announced_name(platform)),
        device: Some(Arc::clone(&key)),
        cancel: Some(cancel.signal()),
        ..ListenOptions::default()
    })?;
    let ticket = offer(&mut listener)?;
    let code = ticket.code.clone();
    on_code(PairInvite {
        ticket,
        key: key.public(),
    });
    let pending = listener.accept_pair_only(&code, &me(platform), CODE_LIFETIME, || {})?;
    // Dropping `pending` unconfirmed fails the pairing on the peer too.
    let peer = &pending.peer;
    let asked = Instant::now();
    let yes = on_peer(&PeerFound::new(&peer.name, peer.os, peer.identity.public));
    cancel.check()?;
    if !yes || asked.elapsed() > PAIR_CONFIRM_WINDOW {
        return Err(CoreError::PairRejected);
    }
    let stored = store(platform, peer)?;
    if let Err(e) = pending.confirm() {
        stored.undo(platform);
        return Err(e.into());
    }
    Ok(stored.record)
}

/// Join the computer whose QR `invite` was scanned; fails with
/// [`CoreError::KeyMismatch`](crate::CoreError::KeyMismatch) if the one
/// reached does not hold the invite's key.
pub fn accept_invite(
    engine: &Engine,
    invite: &PairInvite,
    cancel: &Cancel,
) -> Result<DeviceRecord> {
    join(engine, &invite.ticket, Some(&invite.key), cancel)
}

/// Join with a typed `ticket` alone: whoever answers is trusted on the
/// strength of the code.
pub fn link_with_code(engine: &Engine, ticket: &Ticket, cancel: &Cancel) -> Result<DeviceRecord> {
    join(engine, ticket, None, cancel)
}

fn join(
    engine: &Engine,
    ticket: &Ticket,
    pin: Option<&DevicePublic>,
    cancel: &Cancel,
) -> Result<DeviceRecord> {
    let platform = &**engine.platform();
    let key = KnownDevices::open(platform)?.device_key(platform)?;
    let peer = pair_only(
        ticket,
        pin,
        &me(platform),
        PAIR_TIMEOUT,
        &ConnectOptions {
            platform: Arc::clone(engine.platform()),
            device: Some(Arc::new(key)),
            cancel: Some(cancel.signal()),
        },
    )?;
    Ok(store(platform, &peer)?.record)
}

/// What this device tells the other.
fn me(platform: &dyn Platform) -> PeerInfo {
    PeerInfo {
        name: announced_name(platform),
        os: platform.os(),
    }
}

/// `name` cleaned for display, or `public`'s fingerprint if nothing is left.
fn display_name(name: &str, public: &DevicePublic) -> String {
    clean_device_name(name).unwrap_or_else(|| Fingerprint::of(public).to_string())
}

/// A stored peer, and what it replaced.
struct Stored {
    record: DeviceRecord,
    /// The record and secret a re-paired device had; `None` for a new one.
    prior: Option<(DeviceRecord, KnownSecret)>,
}

impl Stored {
    /// Undo [`store`] after the peer did not complete the pairing: forget a
    /// new device (record, secret, policy), restore a re-paired one. Best
    /// effort: a failure only leaves a device the peer does not know.
    fn undo(self, platform: &dyn Platform) {
        let result = KnownDevices::open(platform).and_then(|known| match self.prior {
            None => known.forget(platform, &self.record.public).map(drop),
            Some((record, secret)) => known.remember(platform, record, &secret),
        });
        if let Err(e) = result {
            tracing::warn!(error = %e, "could not undo an unfinished pairing");
        }
    }
}

/// Remember `peer` and, if it has no policy yet, write its starting one (a
/// phone starts on the phone defaults; a re-paired device keeps its own).
/// Remembered first: with no policy file a device reads as all-off.
fn store(platform: &dyn Platform, peer: &PairedPeer) -> Result<Stored> {
    let public = peer.identity.public;
    let known = KnownDevices::open(platform)?;
    let prior = match known
        .list(platform)?
        .into_iter()
        .find(|d| d.public == public)
    {
        Some(record) => known.secret_for(platform, &public)?.map(|s| (record, s)),
        None => None,
    };
    let record = DeviceRecord {
        public,
        name: display_name(&peer.name, &public),
        os: peer.os,
        first_seen: time::now_unix(),
        last_move: None,
        moves: 0,
        mac_hint: None,
    };
    known.remember(platform, record, &peer.identity.secret)?;
    let stored = Stored {
        record: known
            .list(platform)?
            .into_iter()
            .find(|d| d.public == public)
            .ok_or(CoreError::UnknownDevice)?,
        prior,
    };
    let policy = match peer.os {
        OsKind::Android => PolicyDoc::phone_defaults(),
        _ => PolicyDoc::computer_defaults(),
    };
    let written = PolicyStore::open(platform)
        .and_then(|policies| policies.put_if_absent(platform, &public, &policy));
    if let Err(e) = written {
        stored.undo(platform);
        return Err(e);
    }
    Ok(stored)
}

#[cfg(test)]
#[path = "../../tests/ecosystem/pair/unit.rs"]
mod tests;
