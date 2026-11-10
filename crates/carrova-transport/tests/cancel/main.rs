//! Cancellation over loopback: the caller's signal stops blocking calls,
//! and a cancelled peer is reported as such (never as a lost connection).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use carrova_crypto::pairing::PairingCode;
use carrova_manifest::{ChunkRef, ChunkSource, ChunkSourceError};
use carrova_transport::{
    AllowedChunks, CancelSignal, ConnectOptions, ListenOptions, Listener, TransportError,
};

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod support;
use support::*;

/// A fresh, unset flag.
fn flag() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

fn signal(flag: &Arc<AtomicBool>) -> Option<Arc<dyn CancelSignal>> {
    Some(flag.clone())
}

fn cancellable_listener(flag: &Arc<AtomicBool>) -> Listener {
    Listener::bind(ListenOptions {
        discovery: false,
        cancel: signal(flag),
        ..ListenOptions::default()
    })
    .unwrap()
}

/// Set `flag` after `delay`, from another thread.
fn set_later(flag: &Arc<AtomicBool>, delay: Duration) {
    let flag = flag.clone();
    thread::spawn(move || {
        thread::sleep(delay);
        flag.store(true, Ordering::Release);
    });
}

/// Chunks to transfer: their bytes, references and the sender's allow-list.
struct Chunks {
    all: Vec<(ChunkRef, Vec<u8>)>,
    refs: Vec<ChunkRef>,
    allowed: AllowedChunks,
}

fn chunks(seed: u8) -> Chunks {
    let all = make_chunks(seed, 300, 16 * 1024);
    let refs: Vec<ChunkRef> = all.iter().map(|(c, _)| *c).collect();
    let allowed = AllowedChunks::from_manifest(&manifest(&refs));
    Chunks { all, refs, allowed }
}

/// A source slow enough that a serve is still running when cancelled.
struct Slow(MemSource);

impl ChunkSource for Slow {
    fn read_chunk(&mut self, chunk: &ChunkRef) -> Result<Vec<u8>, ChunkSourceError> {
        thread::sleep(Duration::from_millis(2));
        self.0.read_chunk(chunk)
    }
}

#[test]
fn receiver_cancel_ends_the_fetch_phase_cleanly_on_both_sides() {
    let Chunks { all, refs, allowed } = chunks(1);
    let cancel = flag();
    let opts = ConnectOptions {
        cancel: signal(&cancel),
        ..ConnectOptions::default()
    };
    let (sender, mut receiver, _listener) = pair_with(direct_listener(), &opts);
    let server = serve(sender, allowed, MemSource::new(&all));
    let seen = AtomicUsize::new(0);

    let fetched = receiver.fetch_chunks(&refs, 4, &mut |_, _| {
        if seen.fetch_add(1, Ordering::Relaxed) == 9 {
            cancel.store(true, Ordering::Release);
        }
        Ok(())
    });

    assert!(
        matches!(fetched, Err(TransportError::Cancelled)),
        "{fetched:?}"
    );
    assert!(seen.load(Ordering::Relaxed) < refs.len());
    let (served, mut sender) = server.join().unwrap();
    served.unwrap();
    // The control stream still carries a farewell after the cancel.
    let msg = Ping { n: 1, blob: vec![] };
    receiver.send(&msg).unwrap();
    assert_eq!(sender.recv::<Ping>(T).unwrap(), msg);
}

#[test]
fn sender_cancel_mid_serve_reaches_the_receiver_as_peer_cancelled() {
    let Chunks { all, refs, allowed } = chunks(2);
    let cancel = flag();
    let (sender, mut receiver, _listener) =
        pair_with(cancellable_listener(&cancel), &ConnectOptions::default());
    let server = thread::spawn(move || {
        let mut sender = sender;
        let served = sender.serve_chunks(&allowed, &mut Slow(MemSource::new(&all)));
        sender.close_cancelled();
        served
    });
    let seen = AtomicUsize::new(0);

    let fetched = receiver.fetch_chunks(&refs, 4, &mut |_, _| {
        if seen.fetch_add(1, Ordering::Relaxed) == 9 {
            cancel.store(true, Ordering::Release);
        }
        Ok(())
    });

    assert!(
        matches!(fetched, Err(TransportError::PeerCancelled)),
        "{fetched:?}"
    );
    let served = server.join().unwrap();
    assert!(
        matches!(served, Err(TransportError::Cancelled)),
        "{served:?}"
    );
}

#[test]
fn cancel_stops_waiting_for_a_receiver_promptly() {
    let cancel = flag();
    let mut listener = cancellable_listener(&cancel);
    let code = PairingCode::generate().unwrap();
    set_later(&cancel, Duration::from_millis(100));
    let started = Instant::now();

    let accepted = listener.accept_pairing(&code, Duration::from_secs(30));

    assert!(
        matches!(accepted, Err(TransportError::Cancelled)),
        "{accepted:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn cancel_stops_waiting_for_a_control_message() {
    let cancel = flag();
    let opts = ConnectOptions {
        cancel: signal(&cancel),
        ..ConnectOptions::default()
    };
    let (_sender, mut receiver, _listener) = pair_with(direct_listener(), &opts);
    set_later(&cancel, Duration::from_millis(100));

    let received = receiver.recv::<Ping>(Duration::from_secs(30));

    assert!(
        matches!(received, Err(TransportError::Cancelled)),
        "{received:?}"
    );
}

#[test]
fn a_peer_closing_cancelled_is_peer_cancelled_not_lost() {
    let (sender, mut receiver) = pair();

    sender.close_cancelled();

    let received = receiver.recv::<Ping>(T);
    assert!(
        matches!(received, Err(TransportError::PeerCancelled)),
        "{received:?}"
    );
}
