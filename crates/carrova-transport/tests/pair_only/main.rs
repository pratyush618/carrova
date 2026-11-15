//! Pair-only handshake and QR key pin (ADR 0002 §1–2), through the public
//! API.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::PairingCode;
use carrova_manifest::MAX_DEVICE_NAME;
use carrova_platform::OsKind;
use carrova_transport::{
    ConnectOptions, ListenOptions, Listener, PAIR_CONFIRM_TIMEOUT, PairedPeer, PeerInfo,
    PendingPair, ProtocolViolation, Result, SetupFailure, Target, TransportError, connect,
    pair_only,
};

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod support;
use support::{T, ticket};

fn device() -> Arc<DeviceKey> {
    Arc::new(DeviceKey::generate().unwrap())
}

fn info(name: &str, os: OsKind) -> PeerInfo {
    PeerInfo {
        name: name.into(),
        os,
    }
}

fn listener(device: Option<Arc<DeviceKey>>) -> Listener {
    Listener::bind(ListenOptions {
        discovery: false,
        device,
        ..ListenOptions::default()
    })
    .unwrap()
}

fn opts(device: Option<Arc<DeviceKey>>) -> ConnectOptions {
    ConnectOptions {
        device,
        ..ConnectOptions::default()
    }
}

/// Run `accept_pair_only` on its own thread and hand its pending pair to
/// `finish` there: `(sender result, receiver result)`.
fn pair_with<S: Send + 'static>(
    sender_key: &Arc<DeviceKey>,
    sender_info: PeerInfo,
    receiver_key: &Arc<DeviceKey>,
    receiver_info: &PeerInfo,
    pin: Option<DevicePublic>,
    finish: impl FnOnce(PendingPair) -> S + Send + 'static,
) -> (Result<S>, Result<PairedPeer>) {
    let mut listener = listener(Some(Arc::clone(sender_key)));
    let code = PairingCode::generate().unwrap();
    let t = ticket(&code, &listener);
    let sender = thread::spawn(move || {
        listener
            .accept_pair_only(&code, &sender_info, T, || {})
            .map(finish)
    });
    let receiver = pair_only(
        &t,
        pin.as_ref(),
        receiver_info,
        T,
        &opts(Some(Arc::clone(receiver_key))),
    );
    (sender.join().unwrap(), receiver)
}

/// Pair-only where the sender confirms (stores, then `Linked`).
fn pair(
    pin: Option<DevicePublic>,
) -> (
    Arc<DeviceKey>,
    Arc<DeviceKey>,
    Result<PairedPeer>,
    Result<PairedPeer>,
) {
    let (s_key, r_key) = (device(), device());
    let (sender, receiver) = pair_with(
        &s_key,
        info("laptop", OsKind::Linux),
        &r_key,
        &info("phone", OsKind::Android),
        pin,
        |pending| {
            let peer = pending.peer.clone();
            pending.confirm().map(|()| peer)
        },
    );
    (s_key, r_key, sender.and_then(|r| r), receiver)
}

#[test]
fn pair_only_returns_both_identities() {
    let (s_key, r_key, sender, receiver) = pair(None);
    let seen_by_sender = sender.unwrap();
    let seen_by_receiver = receiver.unwrap();
    assert_eq!(seen_by_sender.identity.public, r_key.public());
    assert_eq!(seen_by_receiver.identity.public, s_key.public());
    assert_eq!(
        seen_by_sender.identity.secret.expose_bytes(),
        seen_by_receiver.identity.secret.expose_bytes()
    );
    assert_eq!(
        (seen_by_sender.name.as_str(), seen_by_sender.os),
        ("phone", OsKind::Android)
    );
    assert_eq!(
        (seen_by_receiver.name.as_str(), seen_by_receiver.os),
        ("laptop", OsKind::Linux)
    );
}

#[test]
fn pair_only_with_matching_pin_succeeds() {
    let s_key = device();
    let r_key = device();
    let (sender, receiver) = pair_with(
        &s_key,
        info("laptop", OsKind::Linux),
        &r_key,
        &info("phone", OsKind::Android),
        Some(s_key.public()),
        PendingPair::confirm,
    );
    sender.unwrap().unwrap();
    assert_eq!(receiver.unwrap().identity.public, s_key.public());
}

#[test]
fn pair_only_with_wrong_pin_fails_key_mismatch() {
    let impostor = device().public();
    let (_, _, sender, receiver) = pair(Some(impostor));
    assert!(
        matches!(receiver, Err(TransportError::KeyMismatch)),
        "{receiver:?}"
    );
    assert!(
        matches!(sender, Err(TransportError::PairingFailed)),
        "{sender:?}"
    );
}

#[test]
fn dropping_pending_pair_fails_receiver() {
    let (s_key, r_key) = (device(), device());
    let (sender, receiver) = pair_with(
        &s_key,
        info("laptop", OsKind::Linux),
        &r_key,
        &info("phone", OsKind::Android),
        None,
        drop,
    );
    sender.unwrap();
    assert!(
        matches!(receiver, Err(TransportError::PairingFailed)),
        "{receiver:?}"
    );
}

/// Past the 10 s exchange bound: the host is asking its user.
const USER_DELAY: Duration = Duration::from_secs(11);

#[test]
fn pair_only_receiver_waits_past_handshake_timeout_for_linked() {
    let (s_key, r_key) = (device(), device());
    let (sender, receiver) = pair_with(
        &s_key,
        info("laptop", OsKind::Linux),
        &r_key,
        &info("phone", OsKind::Android),
        None,
        |pending| {
            thread::sleep(USER_DELAY);
            pending.confirm()
        },
    );
    sender.unwrap().unwrap();
    assert_eq!(receiver.unwrap().identity.public, s_key.public());
}

#[test]
fn pair_only_drop_after_delay_fails_receiver_stores_nothing() {
    let (s_key, r_key) = (device(), device());
    let (sender, receiver) = pair_with(
        &s_key,
        info("laptop", OsKind::Linux),
        &r_key,
        &info("phone", OsKind::Android),
        None,
        |pending| {
            thread::sleep(USER_DELAY);
            drop(pending);
        },
    );
    sender.unwrap();
    assert!(
        matches!(receiver, Err(TransportError::PairingFailed)),
        "{receiver:?}"
    );
}

#[test]
fn pair_only_confirm_timeout_is_a_minute() {
    assert_eq!(PAIR_CONFIRM_TIMEOUT, Duration::from_secs(60));
}

#[test]
fn over_long_peer_name_is_a_violation() {
    let (s_key, r_key) = (device(), device());
    let long = "é".repeat(MAX_DEVICE_NAME + 1);
    let (sender, receiver) = pair_with(
        &s_key,
        info(&long, OsKind::Linux),
        &r_key,
        &info("phone", OsKind::Android),
        None,
        PendingPair::confirm,
    );
    assert!(
        matches!(
            receiver,
            Err(TransportError::Violation(
                ProtocolViolation::DeviceNameTooLong
            ))
        ),
        "{receiver:?}"
    );
    // The sender's own outcome is unspecified: the receiver closed first.
    drop(sender);
}

#[test]
fn pair_only_needs_a_device_key_on_both_sides() {
    let code = PairingCode::generate().unwrap();
    let mut keyless = listener(None);
    let accepted = keyless.accept_pair_only(&code, &info("laptop", OsKind::Linux), T, || {});
    assert!(
        matches!(
            accepted,
            Err(TransportError::Internal(SetupFailure::NoDeviceKey))
        ),
        "{accepted:?}"
    );
    let t = ticket(&code, &keyless);
    let dialled = pair_only(&t, None, &info("phone", OsKind::Android), T, &opts(None));
    assert!(
        matches!(
            dialled,
            Err(TransportError::Internal(SetupFailure::NoDeviceKey))
        ),
        "{dialled:?}"
    );
}

#[test]
fn move_hello_at_pair_only_listener_is_rejected_without_burning_the_code() {
    let (s_key, r_key) = (device(), device());
    let mut listener = listener(Some(s_key));
    let code = PairingCode::generate().unwrap();
    let t = ticket(&code, &listener);
    let sender = thread::spawn(move || {
        listener
            .accept_pair_only(&code, &info("laptop", OsKind::Linux), T, || {})
            .and_then(PendingPair::confirm)
    });
    let moved = connect(Target::Pairing(&t), T, &opts(Some(Arc::clone(&r_key))));
    assert!(matches!(moved, Err(TransportError::NotFound)), "{moved:?}");
    let paired = pair_only(
        &t,
        None,
        &info("phone", OsKind::Android),
        T,
        &opts(Some(r_key)),
    );
    assert!(paired.is_ok(), "{paired:?}");
    sender.join().unwrap().unwrap();
}

#[test]
fn pair_only_hello_at_move_listener_is_rejected() {
    let mut listener = listener(Some(device()));
    let code = PairingCode::generate().unwrap();
    let t = ticket(&code, &listener);
    let sender = thread::spawn(move || listener.accept_pairing(&code, T, || {}));
    let paired = pair_only(
        &t,
        None,
        &info("phone", OsKind::Android),
        T,
        &opts(Some(device())),
    );
    assert!(
        matches!(paired, Err(TransportError::NotFound)),
        "{paired:?}"
    );
    let moved = connect(Target::Pairing(&t), T, &opts(Some(device())));
    assert!(moved.is_ok(), "{moved:?}");
    assert!(sender.join().unwrap().is_ok());
}

#[test]
fn move_pairing_unchanged() {
    let mut listener = listener(Some(device()));
    let code = PairingCode::generate().unwrap();
    let t = ticket(&code, &listener);
    let sender = thread::spawn(move || listener.accept_pairing(&code, T, || {}));
    let receiver = connect(Target::Pairing(&t), T, &opts(Some(device())));
    let sender = sender.join().unwrap();
    assert!(receiver.unwrap().peer_identity().is_some());
    assert!(sender.unwrap().peer_identity().is_some());
}
