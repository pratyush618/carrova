//! A pair-only offer admits only receivers that prove their device
//! identity, and turning one away leaves the code unclaimed.

use std::sync::Arc;
use std::thread;

use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::PairingCode;
use carrova_platform::OsKind;

use super::T;
use super::raw::{dial, loopback};
use crate::wire::{Intent, PairPurpose};
use crate::{ListenOptions, Listener, PeerInfo, TransportError};

#[test]
fn pair_only_without_identify_rejected() {
    let mut listener = Listener::bind(ListenOptions {
        discovery: false,
        device: Some(Arc::new(DeviceKey::generate().unwrap())),
        ..ListenOptions::default()
    })
    .unwrap();
    let code = PairingCode::generate().unwrap();
    let port = listener.port();
    let nameplate = code.nameplate().get();
    let sender = thread::spawn(move || {
        let me = PeerInfo {
            name: "laptop".into(),
            os: OsKind::Linux,
        };
        listener
            .accept_pair_only(&code, &me, T / 5, || {})
            .map(drop)
    });
    let mut peer = dial(loopback(1), port, T).connected();
    let answer = peer.hello_for(Intent::Pair { nameplate }, false, PairPurpose::PairOnly);
    assert!(peer.turned_away(&answer), "{answer:?}");
    // Not burned: the wait ends as a timeout, not a failed pairing.
    let sent = sender.join().unwrap();
    assert!(matches!(sent, Err(TransportError::Timeout)), "{sent:?}");
}
