//! Linking two devices without moving anything (ADR 0002 §1–2): one side
//! offers a code, the other joins with the scanned invite (key pinned) or
//! the typed code (unpinned). Both remember the other and start it on its
//! default policy.

use std::sync::Arc;

use carrova_crypto::identity::{DevicePublic, Fingerprint};
use carrova_crypto::pairing::PairingCode;
use carrova_link::PolicyDoc;
use carrova_platform::{OsKind, Platform};
use carrova_transport::{
    ConnectOptions, ListenOptions, Listener, PairedPeer, PeerInfo, Ticket, pair_only,
};

use super::{PairInvite, PolicyStore};
use crate::devices::{DeviceRecord, KnownDevices};
use crate::engine::Engine;
use crate::error::Result;
use crate::lan::{CODE_LIFETIME, PAIR_TIMEOUT, clean_device_name};
use crate::presence::announced_name;
use crate::time;

/// Wait for a device to link: show `on_code` the invite (QR and typed
/// code), then pair with whoever answers it. Remembers the peer even when
/// "remember devices" is off (linking is an explicit choice).
///
/// Blocks up to [`CODE_LIFETIME`]. The peer is told only after it is
/// stored here, and that confirmation must follow the pairing at once, so
/// nothing slow runs between the two.
pub fn offer_pairing(engine: &Engine, on_code: impl FnOnce(PairInvite)) -> Result<DeviceRecord> {
    let platform = &**engine.platform();
    let key = Arc::new(KnownDevices::open(platform)?.device_key(platform)?);
    let mut listener = Listener::bind(ListenOptions {
        platform: Arc::clone(engine.platform()),
        device_name: Some(announced_name(platform)),
        device: Some(Arc::clone(&key)),
        ..ListenOptions::default()
    })?;
    let code = PairingCode::generate().map_err(carrova_transport::TransportError::from)?;
    listener.advertise_pairing(&code)?;
    on_code(PairInvite {
        ticket: Ticket {
            code: code.clone(),
            addr: listener.fallback_addr(),
        },
        key: key.public(),
    });
    let pending = listener.accept_pair_only(&code, &me(platform), CODE_LIFETIME, || {})?;
    // Stored before the peer is told; on failure the drop fails it too.
    let record = store(platform, &pending.peer)?;
    pending.confirm()?;
    Ok(record)
}

/// Join the computer whose QR `invite` was scanned; fails with
/// [`CoreError::KeyMismatch`](crate::CoreError::KeyMismatch) if the one
/// reached does not hold the invite's key.
pub fn accept_invite(engine: &Engine, invite: &PairInvite) -> Result<DeviceRecord> {
    join(engine, &invite.ticket, Some(&invite.key))
}

/// Join with a typed `ticket` alone: whoever answers is trusted on the
/// strength of the code.
pub fn link_with_code(engine: &Engine, ticket: &Ticket) -> Result<DeviceRecord> {
    join(engine, ticket, None)
}

fn join(engine: &Engine, ticket: &Ticket, pin: Option<&DevicePublic>) -> Result<DeviceRecord> {
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
            cancel: None,
        },
    )?;
    store(platform, &peer)
}

/// What this device tells the other.
fn me(platform: &dyn Platform) -> PeerInfo {
    PeerInfo {
        name: announced_name(platform),
        os: platform.os(),
    }
}

/// Remember `peer` and write its starting policy (a phone starts on the
/// phone defaults). The policy goes first: a failure leaves no record
/// without one.
fn store(platform: &dyn Platform, peer: &PairedPeer) -> Result<DeviceRecord> {
    let public = peer.identity.public;
    let record = DeviceRecord {
        public,
        name: clean_device_name(&peer.name).unwrap_or_else(|| Fingerprint::of(&public).to_string()),
        os: peer.os,
        first_seen: time::now_unix(),
        last_move: None,
        moves: 0,
        mac_hint: None,
    };
    let policy = match peer.os {
        OsKind::Android => PolicyDoc::phone_defaults(),
        _ => PolicyDoc::computer_defaults(),
    };
    PolicyStore::open(platform)?.put(platform, &public, &policy)?;
    KnownDevices::open(platform)?.remember(platform, record.clone(), &peer.identity.secret)?;
    Ok(record)
}
