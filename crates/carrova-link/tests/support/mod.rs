//! Two linked `Connection`s over loopback, via the real pair-only and link handshakes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::{KnownSecret, PairingCode};
use carrova_link::{LaneGate, LaneKind, RefuseReason};
use carrova_platform::OsKind;
use carrova_transport::{
    ConnectOptions, Connection, KnownPeers, ListenOptions, Listener, PeerInfo, Target, Ticket,
    connect, pair_only,
};

/// Lane gate that admits every lane.
pub struct Open;

impl LaneGate for Open {
    fn admit(&self, _: LaneKind) -> Result<(), RefuseReason> {
        Ok(())
    }
}

/// Generous bound for loopback operations.
pub const T: Duration = Duration::from_secs(10);

struct Known(DevicePublic, KnownSecret);

impl KnownPeers for Known {
    fn candidates(&self) -> Vec<(DevicePublic, KnownSecret)> {
        vec![(self.0, self.1.clone())]
    }
}

fn listener(device: &Arc<DeviceKey>) -> Listener {
    Listener::bind(ListenOptions {
        discovery: false,
        device: Some(Arc::clone(device)),
        ..ListenOptions::default()
    })
    .unwrap()
}

fn opts(device: &Arc<DeviceKey>) -> ConnectOptions {
    ConnectOptions {
        device: Some(Arc::clone(device)),
        ..ConnectOptions::default()
    }
}

fn addr(listener: &Listener) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::LOCALHOST, listener.port())
}

/// Pair two fresh devices, then link them: `(dialer, listener side)`.
pub fn linked() -> (Connection, Connection) {
    let (a, b) = (
        Arc::new(DeviceKey::generate().unwrap()),
        Arc::new(DeviceKey::generate().unwrap()),
    );
    let mut sender = listener(&a);
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
        sender
            .accept_pair_only(&code, &me, T, || {})
            .unwrap()
            .confirm()
    });
    let peer = pair_only(&ticket, None, &info("b"), T, &opts(&b)).unwrap();
    paired.join().unwrap().unwrap();
    let secret = peer.identity.secret;

    let mut listening = listener(&a);
    let at = addr(&listening);
    let known = Arc::new(Known(b.public(), secret.clone()));
    let accepted = thread::spawn(move || listening.accept_link(known, T));
    let dialed = connect(
        Target::Link {
            peer: &a.public(),
            secret: &secret,
            addr: at,
        },
        T,
        &opts(&b),
    )
    .unwrap();
    (dialed, accepted.join().unwrap().unwrap())
}
