//! The accept deadline never cuts a pairing attempt that holds the code:
//! its own result decides whether the code burns.

use std::thread;
use std::time::{Duration, Instant};

use carrova_crypto::pairing::PairingCode;

use super::raw::{dial, loopback};
use crate::wire::{Handshake, Intent};
use crate::{ListenOptions, Listener, TransportError};

/// The sender's accept window in these tests.
const WINDOW: Duration = Duration::from_secs(1);

/// Pair with a sender whose window is [`WINDOW`], sending the receiver's
/// PAKE message before the window ends and finishing after it, using
/// `code` on the receiver. Returns the sender's result, the receiver's
/// result and the listener.
fn pair_across_the_deadline(
    code: &PairingCode,
    typed: &PairingCode,
) -> (
    crate::Result<crate::Connection>,
    crate::Result<()>,
    Listener,
) {
    let mut listener = Listener::bind(ListenOptions {
        discovery: false,
        ..ListenOptions::default()
    })
    .unwrap();
    let port = listener.port();
    let started = Instant::now();
    let sender_code = code.clone();
    let sender = thread::spawn(move || {
        let result = listener.accept_pairing(&sender_code, WINDOW, || {});
        (result, listener)
    });
    let mut peer = dial(loopback(1), port, WINDOW).connected();
    let welcome = peer.hello(Intent::Pair {
        nameplate: typed.nameplate().get(),
    });
    assert!(matches!(welcome, Ok(Handshake::Welcome)), "{welcome:?}");
    let pake = peer.send_pake(typed);
    assert!(
        started.elapsed() < WINDOW,
        "PAKE sent too late for this test"
    );
    thread::sleep((WINDOW + WINDOW / 2).saturating_sub(started.elapsed()));
    let received = peer.finish_pairing(pake);
    let (sent, listener) = sender.join().unwrap();
    (sent, received, listener)
}

#[test]
fn pairing_in_flight_at_the_accept_deadline_completes() {
    let code = PairingCode::generate().unwrap();
    let (sent, received, _) = pair_across_the_deadline(&code, &code);
    assert!(sent.is_ok(), "{sent:?}");
    assert!(received.is_ok(), "{received:?}");
}

#[test]
fn failed_pairing_finishing_after_the_deadline_burns_the_code() {
    let code = PairingCode::generate().unwrap();
    let wrong = super::other_words(&code);
    let (sent, received, mut listener) = pair_across_the_deadline(&code, &wrong);
    assert!(
        matches!(sent, Err(TransportError::PairingFailed)),
        "{sent:?}"
    );
    assert!(received.is_err());
    let again = listener.accept_pairing(&code, WINDOW, || {});
    assert!(
        matches!(again, Err(TransportError::PairingFailed)),
        "{again:?}"
    );
}
