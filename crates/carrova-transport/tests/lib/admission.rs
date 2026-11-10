//! Sender admission: which incoming connections may hold a handshake slot,
//! and which resume `Hello`s are recognised.

use std::thread;
use std::time::{Duration, Instant};

use carrova_crypto::pairing::{PairingCode, ResumeSecret, TagPurpose};

use super::raw::{Dial, dial, loopback, stalled};
use crate::discovery::TagProof;
use crate::tuning::MAX_PENDING_HANDSHAKES;
use crate::wire::{Handshake, Intent};
use crate::{ListenOptions, Listener, TransportError};

/// Short enough for a test, longer than any step below.
const WAIT: Duration = Duration::from_secs(4);
/// A refusal arrives within one loopback round trip; a queued connection
/// would hang until this passes.
const PROMPT: Duration = Duration::from_millis(1500);

fn listener() -> Listener {
    Listener::bind(ListenOptions {
        discovery: false,
        ..ListenOptions::default()
    })
    .unwrap()
}

/// A listener waiting (in the background) to pair with a fresh code.
fn pairing_listener() -> (u16, thread::JoinHandle<crate::Result<crate::Connection>>) {
    let mut listener = listener();
    let port = listener.port();
    let code = PairingCode::generate().unwrap();
    (
        port,
        thread::spawn(move || listener.accept_pairing(&code, WAIT)),
    )
}

/// A listener waiting (in the background) for the session of `secret`.
fn resume_listener(
    secret: &ResumeSecret,
) -> (u16, thread::JoinHandle<crate::Result<crate::Connection>>) {
    let mut listener = listener();
    let port = listener.port();
    let secret = secret.clone();
    (
        port,
        thread::spawn(move || listener.accept_resume(&secret, WAIT)),
    )
}

fn resume_intent(proof: TagProof) -> Intent {
    Intent::Resume {
        nonce: proof.nonce.0,
        tag: proof.tag.0,
    }
}

#[test]
fn connection_beyond_the_busy_handshake_slots_is_refused_immediately() {
    let (port, sender) = pairing_listener();
    let busy = stalled(port, 10, MAX_PENDING_HANDSHAKES);
    let started = Instant::now();
    let extra = dial(loopback(20), port, PROMPT);
    assert!(matches!(extra, Dial::Refused), "{extra:?}");
    assert!(started.elapsed() < PROMPT);
    drop(busy);
    assert!(matches!(
        sender.join().unwrap(),
        Err(TransportError::Timeout)
    ));
}

#[test]
fn second_pending_connection_from_one_address_is_refused() {
    let (port, sender) = pairing_listener();
    let first = stalled(port, 30, 1);
    let second = dial(loopback(30), port, PROMPT);
    assert!(matches!(second, Dial::Refused), "{second:?}");
    // Another address still gets a slot.
    let other = dial(loopback(31), port, PROMPT);
    assert!(matches!(other, Dial::Connected(_)), "{other:?}");
    drop(first);
    assert!(matches!(
        sender.join().unwrap(),
        Err(TransportError::Timeout)
    ));
}

#[test]
fn resume_hello_carrying_an_advertised_tag_is_rejected() {
    let secret = ResumeSecret::from_bytes([5; 32]);
    let (port, sender) = resume_listener(&secret);
    // The value anyone on the LAN can read from the sender's advert.
    let advert = TagProof::fresh(&secret, TagPurpose::Discovery).unwrap();
    let mut peer = dial(loopback(40), port, WAIT).connected();
    let answer = peer.hello(resume_intent(advert));
    assert!(peer.turned_away(&answer), "{answer:?}");
    assert!(matches!(
        sender.join().unwrap(),
        Err(TransportError::Timeout)
    ));
}

#[test]
fn replayed_resume_hello_nonce_is_rejected() {
    let secret = ResumeSecret::from_bytes([6; 32]);
    let (port, sender) = resume_listener(&secret);
    let hello = TagProof::fresh(&secret, TagPurpose::Hello).unwrap();
    let mut first = dial(loopback(50), port, WAIT).connected();
    assert!(matches!(
        first.hello(resume_intent(hello)),
        Ok(Handshake::Welcome)
    ));
    let mut replay = dial(loopback(51), port, WAIT).connected();
    let answer = replay.hello(resume_intent(hello));
    assert!(replay.turned_away(&answer), "{answer:?}");
    drop((first, replay));
    assert!(matches!(
        sender.join().unwrap(),
        Err(TransportError::Timeout)
    ));
}
