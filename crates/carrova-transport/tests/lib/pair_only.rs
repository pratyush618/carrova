//! A pair-only offer admits only receivers that prove their device
//! identity, and turning one away leaves the code unclaimed; a pinned
//! receiver never reveals its identity to the wrong sender.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use std::thread;

use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::{PairingCode, Pake, PakeMessage, PakeRole};
use carrova_platform::OsKind;

use super::T;
use super::raw::{dial, loopback};
use crate::endpoint;
use crate::handshake::binding;
use crate::runtime::runtime;
use crate::tls::Identity;
use crate::wire::{Handshake, Intent, PairPurpose, StreamReader, encode_frame};
use crate::{ConnectOptions, ListenOptions, Listener, PeerInfo, Ticket, TransportError, pair_only};

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

/// A raw sender with an unpinned key runs the pairing up to its own
/// `Identify`; the pinned receiver must close without sending its own.
#[test]
fn wrong_pin_receiver_never_sends_its_identify() {
    let identity = Identity::generate().unwrap();
    let platform = carrova_platform::current();
    let config =
        endpoint::server_config(&identity, endpoint::session_transport().unwrap()).unwrap();
    let ep = endpoint::server(platform.as_ref(), config, 0).unwrap();
    let code = PairingCode::generate().unwrap();
    let ticket = Ticket {
        code: code.clone(),
        addr: Some(SocketAddrV4::new(
            Ipv4Addr::LOCALHOST,
            endpoint::local_v4(&ep).unwrap().port(),
        )),
    };
    let receiver = thread::spawn(move || {
        let me = PeerInfo {
            name: "phone".into(),
            os: OsKind::Android,
        };
        let opts = ConnectOptions {
            device: Some(Arc::new(DeviceKey::generate().unwrap())),
            ..ConnectOptions::default()
        };
        let pin = DeviceKey::generate().unwrap().public();
        pair_only(&ticket, Some(&pin), &me, T, &opts)
    });
    let after_identify = runtime().unwrap().block_on(async {
        let conn = ep.accept().await.unwrap().await.unwrap();
        let (mut send, recv) = conn.accept_bi().await.unwrap();
        let mut recv = StreamReader::new(recv);
        let mut put = async |msgs: &[Handshake]| {
            for m in msgs {
                send.write_all(&encode_frame(m).unwrap()).await.unwrap();
            }
        };
        let hello = recv.expect_frame::<Handshake>().await.unwrap();
        assert!(matches!(hello, Handshake::Hello { identify: true, .. }));
        put(&[Handshake::Welcome]).await;
        let Handshake::Pake(theirs) = recv.expect_frame().await.unwrap() else {
            panic!("PAKE expected");
        };
        let (pake, ours) = Pake::start(&code, PakeRole::Sender);
        let keys = pake.finish(&PakeMessage::from_bytes(theirs)).unwrap();
        let (bind, _) = binding(&conn, &identity, PakeRole::Sender).unwrap();
        let confirm = keys.confirm(PakeRole::Sender, &bind).unwrap();
        put(&[
            Handshake::Pake(ours.into_bytes()),
            Handshake::Confirm(confirm.0),
        ])
        .await;
        let Handshake::Confirm(_) = recv.expect_frame().await.unwrap() else {
            panic!("receiver confirmation expected");
        };
        let device = DeviceKey::generate().unwrap();
        put(&[Handshake::Identify {
            public: *device.public().as_bytes(),
            sig: device.sign_binding(PakeRole::Sender, &bind),
        }])
        .await;
        recv.expect_frame::<Handshake>().await
    });
    assert!(
        matches!(after_identify, Err(TransportError::PairingFailed)),
        "{after_identify:?}"
    );
    let received = receiver.join().unwrap();
    assert!(
        matches!(received, Err(TransportError::KeyMismatch)),
        "{received:?}"
    );
}
