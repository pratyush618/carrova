//! Device identity exchange after a code pairing (ADR 0001 §3), through
//! the public API.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::thread;

use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::PairingCode;
use carrova_transport::{
    ConnectOptions, Connection, ListenOptions, Listener, Result, Target, TransportError, connect,
};

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod support;
use support::{T, ticket};

fn device() -> Arc<DeviceKey> {
    Arc::new(DeviceKey::generate().unwrap())
}

/// Pair over loopback with these device keys (`None`: the side does not
/// remember devices, as an older peer): `(sender, receiver, listener)`.
fn pair(
    sender_device: Option<Arc<DeviceKey>>,
    receiver_device: Option<Arc<DeviceKey>>,
) -> (Result<Connection>, Result<Connection>, Listener) {
    let mut listener = Listener::bind(ListenOptions {
        discovery: false,
        device: sender_device,
        ..ListenOptions::default()
    })
    .unwrap();
    let code = PairingCode::generate().unwrap();
    let t = ticket(&code, &listener);
    let sender = thread::spawn(move || {
        let conn = listener.accept_pairing(&code, T, || {});
        (conn, listener)
    });
    let opts = ConnectOptions {
        device: receiver_device,
        ..ConnectOptions::default()
    };
    let receiver = connect(Target::Pairing(&t), T, &opts);
    let (sender, listener) = sender.join().unwrap();
    (sender, receiver, listener)
}

#[test]
fn code_paired_devices_learn_each_other() {
    let (s_key, r_key) = (device(), device());
    let (sender, receiver, _) = pair(Some(Arc::clone(&s_key)), Some(Arc::clone(&r_key)));
    let (sender, receiver) = (sender.unwrap(), receiver.unwrap());
    let seen_by_sender = sender.peer_identity().expect("sender knows the receiver");
    let seen_by_receiver = receiver.peer_identity().expect("receiver knows the sender");
    assert_eq!(seen_by_sender.public, r_key.public());
    assert_eq!(seen_by_receiver.public, s_key.public());
    assert_eq!(
        seen_by_sender.secret.expose_bytes(),
        seen_by_receiver.secret.expose_bytes()
    );
    assert_ne!(
        seen_by_sender.secret.expose_bytes(),
        sender.resume_secret().expose_bytes()
    );
}

#[test]
fn old_peer_without_identify_still_pairs() {
    for (sender_device, receiver_device) in [(Some(device()), None), (None, Some(device()))] {
        let (sender, receiver, _) = pair(sender_device, receiver_device);
        let (sender, receiver) = (sender.unwrap(), receiver.unwrap());
        assert!(sender.peer_identity().is_none());
        assert!(receiver.peer_identity().is_none());
    }
}

#[test]
fn peer_claiming_this_devices_own_key_fails_pairing() {
    let key = device();
    let (sender, receiver, _) = pair(Some(Arc::clone(&key)), Some(key));
    assert!(
        matches!(sender, Err(TransportError::PairingFailed)),
        "{sender:?}"
    );
    assert!(
        matches!(receiver, Err(TransportError::PairingFailed)),
        "{receiver:?}"
    );
}
