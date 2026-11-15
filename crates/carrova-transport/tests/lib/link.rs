//! A link listener admits each recognised `Hello` once, refuses old
//! protocol versions, and needs the device key as well as the secret.

use std::sync::Arc;
use std::thread;

use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::{Confirmation, KnownSecret, PakeRole, TagNonce};
use quinn::ConnectionError;

use super::T;
use super::raw::{RawPeer, dial, loopback};
use crate::handshake::binding;
use crate::wire::{CloseCode, Handshake, Intent, PairPurpose, RejectReason};
use crate::{KnownPeers, ListenOptions, Listener, PROTOCOL_VERSION, TransportError};

struct Known(Vec<(DevicePublic, KnownSecret)>);

impl KnownPeers for Known {
    fn candidates(&self) -> Vec<(DevicePublic, KnownSecret)> {
        self.0.clone()
    }
}

fn secret() -> KnownSecret {
    KnownSecret::from_bytes([7; 32])
}

/// A link listener (for `T / 5`) that knows `dialer` by [`secret`]: its
/// port and result.
fn link_listener(
    dialer: DevicePublic,
) -> (u16, thread::JoinHandle<crate::Result<crate::Connection>>) {
    let mut listener = Listener::bind(ListenOptions {
        discovery: false,
        device: Some(Arc::new(DeviceKey::generate().unwrap())),
        ..ListenOptions::default()
    })
    .unwrap();
    let port = listener.port();
    let known = Arc::new(Known(vec![(dialer, secret())]));
    (
        port,
        thread::spawn(move || listener.accept_link(known, T / 5)),
    )
}

/// `dialer`'s link intent under [`secret`] with `nonce`.
fn intent(nonce: TagNonce, dialer: &DevicePublic) -> Intent {
    Intent::Link {
        nonce: nonce.0,
        tag: secret().link_tag(&nonce, dialer).unwrap().0,
    }
}

fn hello(proto: u32, intent: Intent, identify: bool) -> Handshake {
    Handshake::Hello {
        proto,
        intent,
        identify,
        purpose: PairPurpose::Move,
    }
}

/// The listener closed `peer` with `LinkFailed` (no `Welcome`, no detail).
fn link_failed(peer: &RawPeer, answer: &crate::Result<Handshake>) -> bool {
    answer.is_err()
        && matches!(
            peer.conn.close_reason(),
            Some(ConnectionError::ApplicationClosed(c))
                if CloseCode::from_wire(c.error_code) == Some(CloseCode::LinkFailed)
        )
}

#[test]
fn link_nonce_replay_rejected() {
    let me = DeviceKey::generate().unwrap();
    let (port, listening) = link_listener(me.public());
    let replayed = hello(
        PROTOCOL_VERSION,
        intent(TagNonce::random().unwrap(), &me.public()),
        true,
    );
    let mut first = dial(loopback(1), port, T).connected();
    first.send(&replayed);
    assert!(matches!(first.recv(), Ok(Handshake::Welcome)));

    let mut second = dial(loopback(2), port, T).connected();
    second.send(&replayed);
    let answer = second.recv();
    assert!(link_failed(&second, &answer), "{answer:?}");
    drop((first, second));
    assert!(listening.join().unwrap().is_err());
}

#[test]
fn unrecognised_hello_does_not_use_up_its_nonce() {
    let me = DeviceKey::generate().unwrap();
    let (port, listening) = link_listener(me.public());
    let nonce = TagNonce::random().unwrap();
    let mut stranger = dial(loopback(1), port, T).connected();
    let bad = Intent::Link {
        nonce: nonce.0,
        tag: [0; 16],
    };
    stranger.send(&hello(PROTOCOL_VERSION, bad, true));
    let answer = stranger.recv();
    assert!(link_failed(&stranger, &answer), "{answer:?}");

    let mut real = dial(loopback(2), port, T).connected();
    real.send(&hello(PROTOCOL_VERSION, intent(nonce, &me.public()), true));
    assert!(matches!(real.recv(), Ok(Handshake::Welcome)));
    drop(real);
    assert!(listening.join().unwrap().is_err());
}

#[test]
fn link_from_old_proto_rejected() {
    let me = DeviceKey::generate().unwrap();
    let (port, listening) = link_listener(me.public());
    let mut old = dial(loopback(1), port, T).connected();
    old.send(&hello(
        PROTOCOL_VERSION - 1,
        intent(TagNonce::random().unwrap(), &me.public()),
        true,
    ));
    let answer = old.recv();
    // The `Reject` frame is best effort (the close may overtake it, as for
    // every turned-away `Hello`): exactly `Reject(Version)`, or else the
    // link close and nothing more.
    assert!(
        matches!(answer, Ok(Handshake::Reject(RejectReason::Version)))
            || link_failed(&old, &answer),
        "{answer:?}"
    );
    assert!(matches!(
        listening.join().unwrap(),
        Err(TransportError::Timeout)
    ));
}

#[test]
fn link_hello_without_identify_rejected() {
    let me = DeviceKey::generate().unwrap();
    let (port, listening) = link_listener(me.public());
    let mut peer = dial(loopback(1), port, T).connected();
    peer.send(&hello(
        PROTOCOL_VERSION,
        intent(TagNonce::random().unwrap(), &me.public()),
        false,
    ));
    let answer = peer.recv();
    assert!(link_failed(&peer, &answer), "{answer:?}");
    assert!(listening.join().unwrap().is_err());
}

/// Holding the link secret is not enough: the `Identify` must carry the
/// key the listener knows the secret for.
#[test]
fn stolen_secret_without_the_device_key_cannot_link() {
    let victim = DeviceKey::generate().unwrap().public();
    let thief = DeviceKey::generate().unwrap();
    let (port, listening) = link_listener(victim);
    let mut peer = dial(loopback(1), port, T).connected();
    peer.send(&hello(
        PROTOCOL_VERSION,
        intent(TagNonce::random().unwrap(), &victim),
        true,
    ));
    assert!(matches!(peer.recv(), Ok(Handshake::Welcome)));
    let (binding, _) = binding(&peer.conn, &peer.identity, PakeRole::Receiver).unwrap();
    let Ok(Handshake::Confirm(theirs)) = peer.recv() else {
        panic!("expected the sender's link proof");
    };
    secret()
        .verify(PakeRole::Sender, &binding, &Confirmation(theirs))
        .unwrap();
    let ours = secret().prove(PakeRole::Receiver, &binding).unwrap();
    peer.send(&Handshake::Confirm(ours.0));
    assert!(matches!(peer.recv(), Ok(Handshake::Identify { .. })));
    peer.send(&Handshake::Identify {
        public: *thief.public().as_bytes(),
        sig: thief.sign_link(PakeRole::Receiver, &binding),
    });
    let answer = peer.recv();
    assert!(link_failed(&peer, &answer), "{answer:?}");
    assert!(matches!(
        listening.join().unwrap(),
        Err(TransportError::Timeout)
    ));
}
