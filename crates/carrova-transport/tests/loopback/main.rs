//! Transport behaviour over loopback: pairing, resume, chunk transfer.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::{KnownSecret, PairingCode, ResumeSecret, TagNonce};
use carrova_manifest::{ChunkHash, ChunkRef, MAX_CHUNK_LEN};
use carrova_transport::{
    AllowedChunks, BROADCAST_PORT, BROADCAST_QUERY, BROADCAST_REPLY, ChunkFailure, ConnectOptions,
    Connection, FetchAbort, KnownTags, ListenOptions, Listener, MAX_CHUNK_RETRIES, Nearby,
    PresenceAdvert, PresenceBeacon, Target, Ticket, TransportError, browse, connect,
};

#[path = "../support/mod.rs"]
mod support;
use support::*;

/// The transport's tunables, so tests size their workloads from them.
#[path = "../../src/tuning.rs"]
#[expect(
    dead_code,
    reason = "tests read only the constants they size workloads from"
)]
mod tuning;

// ------------------------------------------------------------------ pairing

#[test]
fn pairing_succeeds_and_control_messages_round_trip() {
    let (mut sender, mut receiver) = pair();
    assert_eq!(
        sender.resume_secret().expose_bytes(),
        receiver.resume_secret().expose_bytes()
    );
    assert_ne!(sender.peer_cert_hash(), receiver.peer_cert_hash());
    assert_eq!(receiver.peer_addr().ip(), LOOPBACK);
    control_round_trip(&mut receiver, &mut sender);
}

#[test]
fn recv_times_out_without_losing_the_next_message() {
    let (mut sender, mut receiver) = pair();
    assert!(matches!(
        receiver.recv::<Ping>(Duration::from_millis(50)),
        Err(TransportError::Timeout)
    ));
    control_round_trip(&mut sender, &mut receiver);
}

#[test]
fn wrong_code_fails_on_both_sides_and_burns_the_code() {
    let mut listener = direct_listener();
    let code = PairingCode::generate().unwrap();
    let wrong = other_words(&code);
    let t = ticket(&wrong, &listener);
    let sender_code = code.clone();
    let sender = thread::spawn(move || {
        let first = listener.accept_pairing(&sender_code, T, || {});
        (first, listener)
    });
    let receiver = connect(Target::Pairing(&t), T, &ConnectOptions::default());
    let (first, mut listener) = sender.join().unwrap();
    assert!(matches!(receiver, Err(TransportError::PairingFailed)));
    assert!(matches!(first, Err(TransportError::PairingFailed)));

    // The burned code is refused outright, without waiting for anyone.
    let started = Instant::now();
    assert!(matches!(
        listener.accept_pairing(&code, T, || {}),
        Err(TransportError::PairingFailed)
    ));
    assert!(started.elapsed() < Duration::from_secs(1));

    // The sender shows a fresh code; the old (correct) guess no longer works.
    let fresh = other_words(&code);
    let t = ticket(&code, &listener);
    let sender = thread::spawn(move || listener.accept_pairing(&fresh, T, || {}));
    assert!(matches!(
        connect(Target::Pairing(&t), T, &ConnectOptions::default()),
        Err(TransportError::PairingFailed)
    ));
    assert!(matches!(
        sender.join().unwrap(),
        Err(TransportError::PairingFailed)
    ));
}

#[test]
fn wrong_nameplate_is_turned_away_without_burning_the_code() {
    let mut listener = direct_listener();
    let code = PairingCode::generate().unwrap();
    let other_plate = code.nameplate().get() % 999 + 1;
    let words = code.to_string().split_once('-').unwrap().1.to_owned();
    let elsewhere: PairingCode = format!("{other_plate}-{words}").parse().unwrap();
    let wrong_ticket = ticket(&elsewhere, &listener);
    let right_ticket = ticket(&code, &listener);
    let sender = thread::spawn(move || listener.accept_pairing(&code, T, || {}));
    assert!(matches!(
        connect(
            Target::Pairing(&wrong_ticket),
            T,
            &ConnectOptions::default()
        ),
        Err(TransportError::NotFound)
    ));
    let mut receiver = connect(
        Target::Pairing(&right_ticket),
        T,
        &ConnectOptions::default(),
    )
    .unwrap();
    let mut sender = sender.join().unwrap().unwrap();
    control_round_trip(&mut sender, &mut receiver);
}

#[test]
fn ticket_round_trips_and_debug_hides_the_words() {
    let code = PairingCode::generate().unwrap();
    let text = format!("{code}@192.168.1.20:40123");
    let t: Ticket = text.parse().unwrap();
    assert_eq!(t.code, code);
    assert_eq!(t.addr, Some("192.168.1.20:40123".parse().unwrap()));
    assert_eq!(t.to_string(), text);
    let words = code.to_string().split_once('-').unwrap().1.to_owned();
    let debug = format!("{t:?}");
    for w in words.split('-') {
        assert!(!debug.contains(w), "{debug}");
    }
    let bare: Ticket = code.to_string().parse().unwrap();
    assert_eq!(bare.addr, None);
    assert!(matches!(
        format!("{code}@nowhere").parse::<Ticket>(),
        Err(TransportError::InvalidTicket(_))
    ));
}

// ------------------------------------------------------------------ discovery + resume

/// Parallel discovery tests must not share a nameplate: on a collision
/// each receiver guesses at the other test's senders and both fail.
#[test]
fn discovery_test_codes_never_share_a_nameplate() {
    let pool = NameplatePool::new();
    let mut seen = std::collections::HashSet::new();
    for _ in 0..200 {
        assert!(seen.insert(pool.claim().nameplate()));
    }
}

#[test]
fn pairing_finds_the_sender_by_discovery() {
    if !loopback_broadcast_works() {
        return;
    }
    let mut listener = discoverable_listener();
    let code = unique_code();
    listener.advertise_pairing(&code).unwrap();
    let t = Ticket {
        code: code.clone(),
        addr: None,
    };
    let sender = thread::spawn(move || listener.accept_pairing(&code, T, || {}));
    let mut receiver = connect(Target::Pairing(&t), T, &ConnectOptions::default()).unwrap();
    let mut sender = sender.join().unwrap().unwrap();
    control_round_trip(&mut receiver, &mut sender);
}

#[test]
fn the_pairing_observer_fires_once_when_the_receiver_stakes_the_code() {
    let mut listener = direct_listener();
    let code = PairingCode::generate().unwrap();
    let t = ticket(&code, &listener);
    let sender = thread::spawn(move || {
        let mut staked = 0;
        let conn = listener.accept_pairing(&code, T, || staked += 1);
        (conn, staked)
    });
    let receiver = connect(Target::Pairing(&t), T, &ConnectOptions::default());
    let (conn, staked) = sender.join().unwrap();
    assert!(receiver.is_ok() && conn.is_ok());
    assert_eq!(staked, 1);
}

#[test]
fn the_pairing_observer_stays_silent_for_another_nameplate() {
    let mut listener = direct_listener();
    let code = PairingCode::generate().unwrap();
    let other_plate = code.nameplate().get() % 999 + 1;
    let words = code.to_string().split_once('-').unwrap().1.to_owned();
    let elsewhere: PairingCode = format!("{other_plate}-{words}").parse().unwrap();
    let t = ticket(&elsewhere, &listener);
    let sender = thread::spawn(move || {
        let mut staked = 0;
        let result = listener.accept_pairing(&code, Duration::from_secs(2), || staked += 1);
        (result.err(), staked)
    });
    let _ = connect(
        Target::Pairing(&t),
        Duration::from_secs(1),
        &ConnectOptions::default(),
    );
    let (err, staked) = sender.join().unwrap();
    assert!(matches!(err, Some(TransportError::Timeout)), "{err:?}");
    assert_eq!(staked, 0);
}

/// A discoverable listener announcing `name` while it offers a code.
fn named_listener(name: &str) -> Listener {
    Listener::bind(ListenOptions {
        device_name: Some(name.to_owned()),
        ..ListenOptions::default()
    })
    .unwrap()
}

fn browsed() -> Vec<Nearby> {
    browse(&*carrova_platform::current(), Duration::from_secs(2)).unwrap()
}

fn browsed_names() -> Vec<String> {
    browsed()
        .into_iter()
        .filter_map(|n| n.advert.name)
        .collect()
}

#[test]
fn browsing_names_senders_only_while_they_offer_a_code() {
    if !loopback_broadcast_works() {
        return;
    }
    let name = format!("browse-test-{}", unique_code().nameplate());
    let mut listener = named_listener(&name);
    assert!(
        !browsed_names().contains(&name),
        "nothing offered yet: no name advertised"
    );
    listener.advertise_pairing(&unique_code()).unwrap();
    let found = browsed_names();
    assert_eq!(
        found.iter().filter(|n| **n == name).count(),
        1,
        "listed once (distinct senders): {found:?}"
    );
    listener
        .advertise_resume(&ResumeSecret::from_bytes([5; 32]))
        .unwrap();
    assert!(
        !browsed_names().contains(&name),
        "resume adverts never carry the name"
    );
}

#[test]
fn a_presence_is_browsed_by_its_tags_and_names_itself_only_when_told() {
    if !loopback_broadcast_works() {
        return;
    }
    let mut seed = [0u8; 32];
    seed[..16].copy_from_slice(&TagNonce::random().unwrap().0);
    let secret = KnownSecret::from_bytes(seed);
    let me = DeviceKey::from_bytes(&[3; 32]).public();
    let tags = || KnownTags::fresh([&secret], &me).unwrap();
    let name = format!("presence-test-{}", unique_code().nameplate());
    let ours = |found: &[Nearby]| -> Vec<PresenceAdvert> {
        found
            .iter()
            .filter(|n| {
                n.advert
                    .known
                    .as_ref()
                    .is_some_and(|k| k.made_with(&secret, &me))
            })
            .map(|n| n.advert.clone())
            .collect()
    };
    let platform = carrova_platform::current();
    let beacon = PresenceBeacon::start(
        &*platform,
        PresenceAdvert {
            known: tags(),
            ..PresenceAdvert::default()
        },
    )
    .unwrap();
    let found = browsed();
    let quiet = ours(&found);
    assert_eq!(quiet.len(), 1, "{found:?}");
    assert_eq!((quiet[0].name.as_ref(), quiet[0].os), (None, None));

    drop(beacon);
    let beacon = PresenceBeacon::start(
        &*platform,
        PresenceAdvert {
            known: tags(),
            name: Some(name.clone()),
            os: Some(platform.os()),
        },
    )
    .unwrap();
    let open = ours(&browsed());
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].name.as_ref(), Some(&name));
    assert_eq!(open[0].os, Some(platform.os()));
    drop(beacon);
    assert!(!browsed_names().contains(&name), "gone once dropped");
}

#[test]
fn resume_reconnects_with_the_session_secret() {
    if !loopback_broadcast_works() {
        return;
    }
    let (sender, receiver, mut listener) = pair_on(discoverable_listener());
    let secret = sender.resume_secret().clone();
    let receiver_secret = receiver.resume_secret().clone();
    drop((sender, receiver));

    listener.advertise_resume(&secret).unwrap();
    let sender = thread::spawn(move || listener.accept_resume(&secret, T));
    let mut receiver = connect(
        Target::Resume(&receiver_secret),
        T,
        &ConnectOptions::default(),
    )
    .unwrap();
    let mut sender = sender.join().unwrap().unwrap();
    assert_eq!(
        sender.resume_secret().expose_bytes(),
        receiver_secret.expose_bytes()
    );
    control_round_trip(&mut sender, &mut receiver);
}

#[test]
fn resume_with_a_wrong_secret_is_refused() {
    if !loopback_broadcast_works() {
        return;
    }
    let mut listener = discoverable_listener();
    let secret = ResumeSecret::from_bytes([3; 32]);
    let wrong = ResumeSecret::from_bytes([4; 32]);
    listener.advertise_resume(&secret).unwrap();
    let sender = thread::spawn(move || {
        listener
            .accept_resume(&secret, Duration::from_secs(3))
            .err()
    });
    assert!(matches!(
        connect(
            Target::Resume(&wrong),
            Duration::from_secs(2),
            &ConnectOptions::default()
        ),
        Err(TransportError::NotFound)
    ));
    assert!(matches!(
        sender.join().unwrap(),
        Some(TransportError::Timeout)
    ));
}

/// Answers every pairing query for `nameplate` with `port` on this host,
/// like a spoofed sender would; stops when the returned flag is set.
fn spoofed_broadcast_answer(nameplate: u16, port: u16) -> Arc<std::sync::atomic::AtomicBool> {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let socket = carrova_platform::current()
        .bind_broadcast_udp(BROADCAST_PORT)
        .unwrap();
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let wanted = [BROADCAST_QUERY, format!("np={nameplate}").as_bytes()].concat();
    let reply = [BROADCAST_REPLY, port.to_string().as_bytes()].concat();
    let flag = Arc::clone(&stop);
    thread::spawn(move || {
        let mut buf = [0u8; 512];
        while !flag.load(Ordering::Relaxed) {
            if let Ok((n, from)) = socket.recv_from(&mut buf)
                && buf[..n] == wanted[..]
            {
                let _ = socket.send_to(&reply, from);
            }
        }
    });
    stop
}

#[test]
fn spoofed_responders_do_not_break_pairing_with_the_real_sender() {
    if !loopback_broadcast_works() {
        return;
    }
    let code = unique_code();
    // An impostor offering the same nameplate with other words: it answers
    // discovery, says Welcome and sends a confirmation that cannot verify.
    let mut impostor = discoverable_listener();
    let impostor_code = other_words(&code);
    impostor.advertise_pairing(&impostor_code).unwrap();
    let impostor = thread::spawn(move || impostor.accept_pairing(&impostor_code, T, || {}));
    // A spoofed answer pointing at a socket that never speaks QUIC.
    let black_hole = UdpSocket::bind((LOOPBACK, 0)).unwrap();
    let spoof = spoofed_broadcast_answer(
        code.nameplate().get(),
        black_hole.local_addr().unwrap().port(),
    );

    let mut listener = discoverable_listener();
    listener.advertise_pairing(&code).unwrap();
    let t = Ticket {
        code: code.clone(),
        addr: None,
    };
    let sender = thread::spawn(move || listener.accept_pairing(&code, T, || {}));
    let started = Instant::now();
    let mut receiver = connect(Target::Pairing(&t), T, &ConnectOptions::default()).unwrap();
    let mut sender = sender.join().unwrap().unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    control_round_trip(&mut receiver, &mut sender);
    spoof.store(true, Ordering::Relaxed);
    // The impostor saw a PAKE message with the wrong code, or no one.
    assert!(matches!(
        impostor.join().unwrap(),
        Err(TransportError::PairingFailed | TransportError::Timeout)
    ));
}

#[test]
fn discovered_sender_with_a_wrong_code_fails_pairing_promptly() {
    if !loopback_broadcast_works() {
        return;
    }
    let mut listener = discoverable_listener();
    let code = unique_code();
    listener.advertise_pairing(&code).unwrap();
    let t = Ticket {
        code: other_words(&code),
        addr: None,
    };
    let sender = thread::spawn(move || listener.accept_pairing(&code, T, || {}));
    let started = Instant::now();
    assert!(matches!(
        connect(Target::Pairing(&t), T, &ConnectOptions::default()),
        Err(TransportError::PairingFailed)
    ));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "waited for the full timeout"
    );
    assert!(matches!(
        sender.join().unwrap(),
        Err(TransportError::PairingFailed)
    ));
}

/// `n` discoverable impostors offering `code`'s nameplate with other words,
/// each waiting `wait` for one pairing attempt in the background.
fn impostors(
    code: &PairingCode,
    n: usize,
    wait: Duration,
) -> Vec<thread::JoinHandle<Result<Connection, TransportError>>> {
    (0..n)
        .map(|_| {
            let mut impostor = discoverable_listener();
            let impostor_code = other_words(code);
            impostor.advertise_pairing(&impostor_code).unwrap();
            thread::spawn(move || impostor.accept_pairing(&impostor_code, wait, || {}))
        })
        .collect()
}

/// How many impostors received a PAKE message (a guess at their code).
fn guesses_received(
    impostors: Vec<thread::JoinHandle<Result<Connection, TransportError>>>,
) -> usize {
    impostors
        .into_iter()
        .map(|i| i.join().unwrap())
        .filter(|r| matches!(r, Err(TransportError::PairingFailed)))
        .count()
}

#[test]
fn receiver_spends_at_most_three_code_guesses_per_connect() {
    if !loopback_broadcast_works() {
        return;
    }
    let code = unique_code();
    let crowd = impostors(&code, 6, Duration::from_secs(6));
    let t = Ticket {
        code: code.clone(),
        addr: None,
    };
    let result = connect(
        Target::Pairing(&t),
        Duration::from_secs(4),
        &ConnectOptions::default(),
    );
    assert!(
        matches!(result, Err(TransportError::PairingFailed)),
        "{result:?}"
    );
    let guesses = guesses_received(crowd);
    assert!(guesses <= 3, "{guesses} impostors received a guess");
}

#[test]
fn real_sender_among_impostors_pairs_within_the_guess_budget() {
    if !loopback_broadcast_works() {
        return;
    }
    let code = unique_code();
    let crowd = impostors(&code, 1, Duration::from_secs(6));
    let mut listener = discoverable_listener();
    listener.advertise_pairing(&code).unwrap();
    let t = Ticket {
        code: code.clone(),
        addr: None,
    };
    let sender = thread::spawn(move || listener.accept_pairing(&code, T, || {}));
    let mut receiver = connect(Target::Pairing(&t), T, &ConnectOptions::default()).unwrap();
    let mut sender = sender.join().unwrap().unwrap();
    control_round_trip(&mut receiver, &mut sender);
    assert!(guesses_received(crowd) <= 1);
}

// ------------------------------------------------------------------ chunks

/// Most bytes one fetch with `parallel` streams has requested or holds
/// undelivered at once: chunks within it may arrive in any order.
fn in_flight_window(parallel: usize) -> usize {
    parallel * tuning::STREAM_OUTSTANDING_BYTES + tuning::DELIVERY_BUDGET_BYTES
}

#[test]
fn fetch_delivers_every_chunk_roughly_in_priority_order() {
    const BIG: usize = 256 * 1024;
    // Twice the window in big chunks, then small ones: the last quarter
    // (all small) cannot be requested until the window has drained, so
    // the order below is decided by priority, not by luck.
    let window = in_flight_window(carrova_transport::DEFAULT_PARALLEL_FETCHES);
    let mut all = make_chunks(1, (2 * window).div_ceil(BIG), BIG);
    all.extend(make_chunks(2, 2000, 700));
    let refs: Vec<ChunkRef> = all.iter().map(|(c, _)| *c).collect();
    let expected: HashMap<ChunkHash, Vec<u8>> =
        all.iter().map(|(c, b)| (c.hash, b.clone())).collect();
    let (sender, mut receiver) = pair();
    let server = serve(
        sender,
        AllowedChunks::from_manifest(&manifest(&refs)),
        MemSource::new(&all),
    );

    let position: HashMap<ChunkHash, usize> =
        refs.iter().enumerate().map(|(i, c)| (c.hash, i)).collect();
    let mut delivered = Vec::new();
    let report = receiver
        .fetch_chunks(
            &refs,
            carrova_transport::DEFAULT_PARALLEL_FETCHES,
            &mut |c, bytes| {
                assert_eq!(&bytes, &expected[&c.hash]);
                delivered.push(position[&c.hash]);
                Ok(())
            },
        )
        .unwrap();
    let (end, _sender) = server.join().unwrap();
    let end = end.unwrap();

    assert_eq!(report.chunks, refs.len() as u64);
    assert!(report.failed.is_empty());
    assert_eq!(end.chunks, refs.len() as u64);
    assert_eq!(report.bytes, end.bytes);
    let mut sorted = delivered.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..refs.len()).collect::<Vec<_>>());
    // Priority: the first quarter requested is, on average, delivered well
    // before the last quarter.
    let quarter = refs.len() / 4;
    let mean_rank = |range: std::ops::Range<usize>| {
        let ranks: Vec<usize> = delivered
            .iter()
            .enumerate()
            .filter(|(_, p)| range.contains(p))
            .map(|(rank, _)| rank)
            .collect();
        ranks.iter().sum::<usize>() / ranks.len()
    };
    assert!(mean_rank(0..quarter) < mean_rank(refs.len() - quarter..refs.len()));
}

#[test]
fn tampered_chunk_is_retried_then_reported_failed() {
    let all = make_chunks(3, 20, 4096);
    let refs: Vec<ChunkRef> = all.iter().map(|(c, _)| *c).collect();
    let bad = refs[5];
    let mut source = MemSource::new(&all);
    source.corrupt = Some(bad.hash);
    let reads = Arc::clone(&source.reads);
    let (sender, mut receiver) = pair();
    let server = serve(
        sender,
        AllowedChunks::from_manifest(&manifest(&refs)),
        source,
    );

    let report = receiver
        .fetch_chunks(&refs, 4, &mut |c, _| {
            assert_ne!(c.hash, bad.hash, "a corrupt chunk reached the callback");
            Ok(())
        })
        .unwrap();
    server.join().unwrap().0.unwrap();

    assert_eq!(report.chunks, refs.len() as u64 - 1);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].chunk, bad);
    assert_eq!(report.failed[0].reason, ChunkFailure::VerificationFailed);
    assert_eq!(
        reads.lock().unwrap()[&bad.hash],
        1 + MAX_CHUNK_RETRIES as usize
    );
}

#[test]
fn chunk_the_sender_cannot_read_is_reported_unavailable() {
    let all = make_chunks(4, 10, 1000);
    let refs: Vec<ChunkRef> = all.iter().map(|(c, _)| *c).collect();
    let mut source = MemSource::new(&all);
    source.chunks.remove(&refs[3].hash);
    let (sender, mut receiver) = pair();
    let server = serve(
        sender,
        AllowedChunks::from_manifest(&manifest(&refs)),
        source,
    );
    let report = receiver.fetch_chunks(&refs, 2, &mut |_, _| Ok(())).unwrap();
    server.join().unwrap().0.unwrap();
    assert_eq!(report.chunks, 9);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].reason, ChunkFailure::Unavailable);
}

#[test]
fn request_for_a_chunk_outside_the_manifest_is_rejected() {
    let all = make_chunks(5, 4, 1000);
    let refs: Vec<ChunkRef> = all.iter().map(|(c, _)| *c).collect();
    let (sender, mut receiver) = pair();
    // The sender only offers the first three chunks.
    let server = serve(
        sender,
        AllowedChunks::from_manifest(&manifest(&refs[..3])),
        MemSource::new(&all),
    );
    let fetched = receiver.fetch_chunks(&refs[3..], 1, &mut |_, _| Ok(()));
    let (served, _sender) = server.join().unwrap();
    assert!(matches!(served, Err(TransportError::Violation(_))));
    assert!(matches!(fetched, Err(TransportError::Violation(_))));
}

#[test]
fn chunk_longer_than_the_manifest_limit_is_refused() {
    let bytes = vec![7u8; MAX_CHUNK_LEN as usize + 1];
    let chunk = ChunkRef {
        hash: ChunkHash::of(&bytes),
        offset: 0,
        len: MAX_CHUNK_LEN + 1,
    };
    let (sender, mut receiver) = pair();
    let server = serve(
        sender,
        AllowedChunks::from_manifest(&manifest(&[chunk])),
        MemSource::new(&[(chunk, bytes)]),
    );
    let fetched = receiver.fetch_chunks(&[chunk], 1, &mut |_, _| Ok(()));
    let (served, _sender) = server.join().unwrap();
    assert!(
        matches!(fetched, Err(TransportError::Violation(_))),
        "{fetched:?}"
    );
    assert!(
        matches!(served, Err(TransportError::Violation(_))),
        "{served:?}"
    );
}

#[test]
fn re_requesting_beyond_three_times_the_manifest_bytes_is_refused() {
    let all = make_chunks(9, 1, 4096);
    let refs: Vec<ChunkRef> = all.iter().map(|(c, _)| *c).collect();
    let allowed = AllowedChunks::from_manifest(&manifest(&refs));
    let (mut sender, mut receiver) = pair();
    let server = thread::spawn(move || {
        let mut source = MemSource::new(&all);
        loop {
            if let Err(e) = sender.serve_chunks(&allowed, &mut source) {
                return e;
            }
        }
    });
    for _ in 0..3 {
        let report = receiver.fetch_chunks(&refs, 1, &mut |_, _| Ok(())).unwrap();
        assert_eq!(report.chunks, 1);
    }
    assert!(matches!(
        receiver.fetch_chunks(&refs, 1, &mut |_, _| Ok(())),
        Err(TransportError::Violation(_))
    ));
    assert!(matches!(
        server.join().unwrap(),
        TransportError::Violation(_)
    ));
}

#[test]
fn sender_dropping_mid_fetch_is_peer_lost() {
    let all = make_chunks(6, 400, 16 * 1024);
    let refs: Vec<ChunkRef> = all.iter().map(|(c, _)| *c).collect();
    let mut source = MemSource::new(&all);
    source.crash_after = Some(100);
    let (sender, mut receiver) = pair();
    let server = serve(
        sender,
        AllowedChunks::from_manifest(&manifest(&refs)),
        source,
    );
    let mut delivered = 0;
    let started = Instant::now();
    let fetched = receiver.fetch_chunks(&refs, 4, &mut |_, _| {
        delivered += 1;
        Ok(())
    });
    assert!(server.join().is_err(), "the sender thread should have died");
    assert!(
        matches!(fetched, Err(TransportError::PeerLost)),
        "{fetched:?}"
    );
    assert!(delivered <= 100);
    // Noticed from the close, not the idle timeout.
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn fetch_abort_stops_cleanly_and_the_connection_stays_usable() {
    let all = make_chunks(7, 300, 16 * 1024);
    let refs: Vec<ChunkRef> = all.iter().map(|(c, _)| *c).collect();
    let (sender, mut receiver) = pair();
    let server = serve(
        sender,
        AllowedChunks::from_manifest(&manifest(&refs)),
        MemSource::new(&all),
    );
    let seen = AtomicUsize::new(0);
    let fetched = receiver.fetch_chunks(&refs, 4, &mut |_, _| {
        if seen.fetch_add(1, Ordering::Relaxed) == 9 {
            return Err(FetchAbort::OutOfSpace);
        }
        Ok(())
    });
    assert!(matches!(
        fetched,
        Err(TransportError::Aborted(FetchAbort::OutOfSpace))
    ));
    assert_eq!(seen.load(Ordering::Relaxed), 10);
    let (served, mut sender) = server.join().unwrap();
    served.unwrap();
    control_round_trip(&mut receiver, &mut sender);
}

#[test]
fn bulk_transfer_throughput_over_loopback() {
    const CHUNK: usize = 256 * 1024;
    const TOTAL: usize = 256 * 1024 * 1024;
    let all = make_chunks(8, TOTAL / CHUNK, CHUNK);
    let refs: Vec<ChunkRef> = all.iter().map(|(c, _)| *c).collect();
    let (sender, mut receiver) = pair();
    let server = serve(
        sender,
        AllowedChunks::from_manifest(&manifest(&refs)),
        MemSource::new(&all),
    );
    drop(all);
    let started = Instant::now();
    let report = receiver
        .fetch_chunks(
            &refs,
            carrova_transport::DEFAULT_PARALLEL_FETCHES,
            &mut |_, _| Ok(()),
        )
        .unwrap();
    let elapsed = started.elapsed();
    server.join().unwrap().0.unwrap();
    assert_eq!(report.bytes, TOTAL as u64);
    let mb_s = TOTAL as f64 / 1e6 / elapsed.as_secs_f64();
    eprintln!("loopback throughput: {mb_s:.0} MB/s ({TOTAL} bytes in {elapsed:?})");
    if !cfg!(debug_assertions) {
        assert!(mb_s > 100.0, "{mb_s:.0} MB/s");
    }
}
