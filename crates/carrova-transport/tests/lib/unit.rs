//! In-crate behaviour tests that need raw (unauthenticated) QUIC access.

mod admission;
mod deadline;
mod raw;

use std::net::{Ipv4Addr, SocketAddrV4};
use std::thread;
use std::time::{Duration, Instant};

use carrova_crypto::pairing::PairingCode;

use crate::tuning::{MAX_PENDING_HANDSHAKES, PRE_PAKE_TIMEOUT};
use crate::{ConnectOptions, ListenOptions, Listener, Target, Ticket, TransportError, connect};

const T: Duration = Duration::from_secs(10);

fn listener() -> Listener {
    Listener::bind(ListenOptions {
        discovery: false,
        ..ListenOptions::default()
    })
    .unwrap()
}

#[test]
fn stalled_connections_do_not_block_a_legitimate_pairing() {
    let mut listener = listener();
    let code = PairingCode::generate().unwrap();
    let ticket = Ticket {
        code: code.clone(),
        addr: Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, listener.port())),
    };
    let sender = thread::spawn(move || listener.accept_pairing(&code, T));
    // Fill every handshake slot (one per source address).
    let stalled = raw::stalled(ticket.addr.unwrap().port(), 60, MAX_PENDING_HANDSHAKES);
    let started = Instant::now();
    let receiver = connect(Target::Pairing(&ticket), T, &ConnectOptions::default());
    let elapsed = started.elapsed();
    let sender = sender.join().unwrap();
    drop(stalled);
    assert!(receiver.is_ok(), "{receiver:?}");
    assert!(sender.is_ok(), "{sender:?}");
    // Waited at most for the stalled slots' pre-PAKE timeouts.
    assert!(
        elapsed < 2 * PRE_PAKE_TIMEOUT + Duration::from_secs(2),
        "{elapsed:?}"
    );
}

#[test]
fn stalled_connections_do_not_burn_the_code() {
    let mut listener = listener();
    let code = PairingCode::generate().unwrap();
    let port = listener.port();
    let sender_code = code.clone();
    // Only stalled peers: the wait ends as a timeout, the code survives.
    let sender = thread::spawn(move || {
        let first = listener.accept_pairing(&sender_code, 2 * PRE_PAKE_TIMEOUT);
        (first, listener)
    });
    let stalled = raw::stalled(port, 70, 2);
    let (first, mut listener) = sender.join().unwrap();
    drop(stalled);
    assert!(matches!(first, Err(TransportError::Timeout)), "{first:?}");
    let ticket = Ticket {
        code: code.clone(),
        addr: Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)),
    };
    let sender = thread::spawn(move || listener.accept_pairing(&code, T));
    assert!(connect(Target::Pairing(&ticket), T, &ConnectOptions::default()).is_ok());
    assert!(sender.join().unwrap().is_ok());
}

/// A different code on the same nameplate.
fn other_words(code: &PairingCode) -> PairingCode {
    loop {
        let fresh = PairingCode::generate().unwrap().to_string();
        let words = fresh.split_once('-').unwrap().1;
        let other: PairingCode = format!("{}-{words}", code.nameplate()).parse().unwrap();
        if &other != code {
            return other;
        }
    }
}
