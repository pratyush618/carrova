//! Code-less link handshake between paired devices (ADR 0002 §3–5),
//! through the public API.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::{KnownSecret, PairingCode};
use carrova_platform::OsKind;
use carrova_transport::{
    ConnectOptions, Connection, KnownPeers, ListenOptions, Listener, PeerInfo, Result,
    SetupFailure, Target, Ticket, TransportError, connect, pair_only,
};

const T: Duration = Duration::from_secs(10);
/// How long a listener waits in tests that expect no link.
const SHORT: Duration = Duration::from_secs(2);

/// Linkable peers; a test may forget one while a listener runs.
struct Known(Mutex<Vec<(DevicePublic, KnownSecret)>>);

impl Known {
    fn new(peers: Vec<(DevicePublic, KnownSecret)>) -> Arc<Self> {
        Arc::new(Self(Mutex::new(peers)))
    }

    fn forget(&self, peer: &DevicePublic) {
        self.0.lock().unwrap().retain(|(public, _)| public != peer);
    }
}

impl KnownPeers for Known {
    fn candidates(&self) -> Vec<(DevicePublic, KnownSecret)> {
        self.0.lock().unwrap().clone()
    }
}

fn device() -> Arc<DeviceKey> {
    Arc::new(DeviceKey::generate().unwrap())
}

fn secret(n: u8) -> KnownSecret {
    KnownSecret::from_bytes([n; 32])
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

fn addr(listener: &Listener) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::LOCALHOST, listener.port())
}

/// Run `accept_link` on its own thread for `wait`, knowing `known`.
fn accept(
    mut listener: Listener,
    known: Arc<Known>,
    wait: Duration,
) -> thread::JoinHandle<Result<Connection>> {
    thread::spawn(move || listener.accept_link(known, wait))
}

/// Dial `addr` as the device `me`, expecting `peer` and proving `secret`.
fn dial(
    me: &Arc<DeviceKey>,
    peer: &DevicePublic,
    secret: &KnownSecret,
    addr: SocketAddrV4,
) -> Result<Connection> {
    connect(
        Target::Link { peer, secret, addr },
        T,
        &opts(Some(Arc::clone(me))),
    )
}

/// Pair `a` (sender) and `b` without a transfer: the secret both stored.
fn pair_only_secret(a: &Arc<DeviceKey>, b: &Arc<DeviceKey>) -> KnownSecret {
    let mut sender = listener(Some(Arc::clone(a)));
    let code = PairingCode::generate().unwrap();
    let ticket = Ticket {
        code: code.clone(),
        addr: Some(addr(&sender)),
    };
    let info = |name: &str| PeerInfo {
        name: name.into(),
        os: OsKind::Linux,
    };
    let me = info("a");
    let paired = thread::spawn(move || {
        let pending = sender.accept_pair_only(&code, &me, T, || {})?;
        pending.confirm()
    });
    let peer = pair_only(&ticket, None, &info("b"), T, &opts(Some(Arc::clone(b)))).unwrap();
    paired.join().unwrap().unwrap();
    peer.identity.secret
}

#[test]
fn link_after_pair_only_connects() {
    let (a, b) = (device(), device());
    let shared = pair_only_secret(&a, &b);
    let listening = listener(Some(Arc::clone(&a)));
    let at = addr(&listening);
    let accepted = accept(listening, Known::new(vec![(b.public(), shared.clone())]), T);
    let mut dialed = dial(&b, &a.public(), &shared, at).unwrap();
    let mut accepted = accepted.join().unwrap().unwrap();

    assert_eq!(dialed.peer_identity().unwrap().public, a.public());
    assert_eq!(accepted.peer_identity().unwrap().public, b.public());
    assert!(dialed.resume_secret().is_none(), "a link is never resumed");
    assert!(accepted.resume_secret().is_none());
    dialed.send(&"ping".to_owned()).unwrap();
    assert_eq!(accepted.recv::<String>(T).unwrap(), "ping");
}

#[test]
fn link_bad_tag_rejected() {
    let (a, b) = (device(), device());
    let listening = listener(Some(Arc::clone(&a)));
    let at = addr(&listening);
    let accepted = accept(listening, Known::new(vec![(b.public(), secret(1))]), T);

    let wrong = dial(&b, &a.public(), &secret(2), at);
    assert!(
        matches!(wrong, Err(TransportError::LinkFailed)),
        "{wrong:?}"
    );
    // The listener keeps accepting: the right secret still links.
    dial(&b, &a.public(), &secret(1), at).unwrap();
    assert_eq!(
        accepted
            .join()
            .unwrap()
            .unwrap()
            .peer_identity()
            .unwrap()
            .public,
        b.public()
    );
}

#[test]
fn link_wrong_device_key_rejected() {
    let (a, b, impostor) = (device(), device(), device());
    // The listener holds the right secret but is not the device we stored.
    let listening = listener(Some(Arc::clone(&impostor)));
    let at = addr(&listening);
    let accepted = accept(listening, Known::new(vec![(b.public(), secret(1))]), SHORT);
    let dialed = dial(&b, &a.public(), &secret(1), at);
    assert!(
        matches!(dialed, Err(TransportError::LinkFailed)),
        "{dialed:?}"
    );
    let accepted = accepted.join().unwrap();
    assert!(
        matches!(accepted, Err(TransportError::Timeout)),
        "{accepted:?}"
    );

    // A dialer with the right secret but another key is not recognised.
    let listening = listener(Some(Arc::clone(&a)));
    let at = addr(&listening);
    let accepted = accept(listening, Known::new(vec![(b.public(), secret(1))]), SHORT);
    let dialed = dial(&impostor, &a.public(), &secret(1), at);
    assert!(
        matches!(dialed, Err(TransportError::LinkFailed)),
        "{dialed:?}"
    );
    assert!(accepted.join().unwrap().is_err());
}

#[test]
fn forgotten_peer_link_rejected() {
    let (a, b) = (device(), device());
    let listening = listener(Some(Arc::clone(&a)));
    let at = addr(&listening);
    let accepted = accept(listening, Known::new(Vec::new()), SHORT);
    let dialed = dial(&b, &a.public(), &secret(1), at);
    assert!(
        matches!(dialed, Err(TransportError::LinkFailed)),
        "{dialed:?}"
    );
    assert!(matches!(
        accepted.join().unwrap(),
        Err(TransportError::Timeout)
    ));
}

#[test]
fn only_the_first_64_candidates_are_tried() {
    let (a, b) = (device(), device());
    let others: Vec<_> = (0..64)
        .map(|n| (device().public(), secret(n + 10)))
        .collect();
    let listening = listener(Some(Arc::clone(&a)));
    let at = addr(&listening);
    let known = Known::new(
        others
            .into_iter()
            .chain([(b.public(), secret(1))])
            .collect(),
    );
    let accepted = accept(listening, known, SHORT);
    let dialed = dial(&b, &a.public(), &secret(1), at);
    assert!(
        matches!(dialed, Err(TransportError::LinkFailed)),
        "{dialed:?}"
    );
    assert!(accepted.join().unwrap().is_err());
}

#[test]
fn link_needs_a_device_key() {
    let a = device();
    let mut listening = listener(None);
    let at = addr(&listening);
    assert!(matches!(
        listening.accept_link(Known::new(Vec::new()), SHORT),
        Err(TransportError::Internal(SetupFailure::NoDeviceKey))
    ));
    let dialed = connect(
        Target::Link {
            peer: &a.public(),
            secret: &secret(1),
            addr: at,
        },
        T,
        &opts(None),
    );
    assert!(
        matches!(
            dialed,
            Err(TransportError::Internal(SetupFailure::NoDeviceKey))
        ),
        "{dialed:?}"
    );
}

#[test]
fn superseded_close_reaches_the_peer() {
    let (a, b) = (device(), device());
    let listening = listener(Some(Arc::clone(&a)));
    let at = addr(&listening);
    let accepted = accept(listening, Known::new(vec![(b.public(), secret(1))]), T);
    let mut dialed = dial(&b, &a.public(), &secret(1), at).unwrap();
    accepted.join().unwrap().unwrap().close_superseded();
    let ended = dialed.recv::<String>(T);
    assert!(
        matches!(ended, Err(TransportError::Superseded)),
        "{ended:?}"
    );
}

#[test]
fn forgetting_a_peer_stops_its_links_in_a_running_listener() {
    let (a, b, c) = (device(), device(), device());
    let listening = listener(Some(Arc::clone(&a)));
    let at = addr(&listening);
    let known = Known::new(vec![(b.public(), secret(1)), (c.public(), secret(2))]);
    let accepted = accept(listening, Arc::clone(&known), T);
    // A failed attempt first, so the listener is surely running.
    let early = dial(&b, &a.public(), &secret(9), at);
    assert!(
        matches!(early, Err(TransportError::LinkFailed)),
        "{early:?}"
    );

    known.forget(&b.public());
    let forgotten = dial(&b, &a.public(), &secret(1), at);
    assert!(
        matches!(forgotten, Err(TransportError::LinkFailed)),
        "{forgotten:?}"
    );
    dial(&c, &a.public(), &secret(2), at).unwrap();
    assert_eq!(
        accepted
            .join()
            .unwrap()
            .unwrap()
            .peer_identity()
            .unwrap()
            .public,
        c.public()
    );
}
