//! A code-paired sender that remembers devices checks the receiver's
//! identity proof, and proves its own only to receivers that ask.

use std::sync::Arc;
use std::thread;

use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::{PairingCode, PakeRole};

use super::T;
use super::raw::{RawPeer, dial, loopback};
use crate::wire::{Handshake, Intent};
use crate::{Connection, ListenOptions, Listener, TransportError};

/// A sender remembering devices, accepting `code` on its own thread, and a
/// raw receiver that sent `Hello { identify }`, its PAKE message and its
/// confirmation. Returns the sender's thread, the receiver, and its
/// channel binding.
fn confirmed_pairing(
    identify: bool,
) -> (
    thread::JoinHandle<crate::Result<Connection>>,
    RawPeer,
    carrova_crypto::pairing::ChannelBinding,
) {
    let mut listener = Listener::bind(ListenOptions {
        discovery: false,
        device: Some(Arc::new(DeviceKey::generate().unwrap())),
        ..ListenOptions::default()
    })
    .unwrap();
    let code = PairingCode::generate().unwrap();
    let port = listener.port();
    let sender_code = code.clone();
    let sender = thread::spawn(move || listener.accept_pairing(&sender_code, T, || {}));
    let mut peer = dial(loopback(1), port, T).connected();
    let nameplate = code.nameplate().get();
    let welcome = peer.hello_with(Intent::Pair { nameplate }, identify);
    assert!(matches!(welcome, Ok(Handshake::Welcome)), "{welcome:?}");
    let pake = peer.send_pake(&code);
    let binding = peer.confirm(pake).unwrap();
    (sender, peer, binding)
}

#[test]
fn identify_with_bad_signature_fails_session() {
    let (sender, mut peer, binding) = confirmed_pairing(true);
    let proof = peer.recv();
    assert!(matches!(proof, Ok(Handshake::Identify { .. })), "{proof:?}");
    let device = DeviceKey::generate().unwrap();
    let mut sig = device.sign_binding(PakeRole::Receiver, &binding);
    sig[0] ^= 1;
    peer.send(&Handshake::Identify {
        public: *device.public().as_bytes(),
        sig,
    });
    let sent = sender.join().unwrap();
    assert!(
        matches!(sent, Err(TransportError::PairingFailed)),
        "{sent:?}"
    );
}

#[test]
fn receiver_that_asks_for_no_identity_gets_none() {
    let (sender, mut peer, _) = confirmed_pairing(false);
    let established = peer.recv();
    assert!(
        matches!(established, Ok(Handshake::Established)),
        "{established:?}"
    );
    assert!(sender.join().unwrap().unwrap().peer_identity().is_none());
}
