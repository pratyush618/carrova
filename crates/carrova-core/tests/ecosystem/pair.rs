//! Pair-only linking: invite (pinned) and typed code (unpinned).

#![allow(clippy::unwrap_used)]

use std::sync::{Arc, mpsc};
use std::thread;

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::ecosystem::{
    PAIR_CONFIRM_WINDOW, PairInvite, PeerFound, PolicyStore, accept_invite, link_with_code,
    offer_pairing,
};
use carrova_core::{Cancel, CoreError, Engine, NullSink};
use carrova_crypto::identity::{DeviceKey, Fingerprint};
use carrova_link::{Feature, FeatureState, PolicyDoc};
use carrova_platform::OsKind;
use carrova_platform::fake::FakePlatform;
use carrova_transport::TransportError;

use crate::lan_support::{Side, loopback};

type HostResult = thread::JoinHandle<carrova_core::Result<DeviceRecord>>;

/// `host` offers a pairing and confirms whoever is found.
fn offer(host: &Side) -> (PairInvite, HostResult) {
    offer_asking(host, |_| true)
}

/// `host` offers a pairing, answering the key check with `on_peer`;
/// returns its invite (pointed at loopback) and the thread that ends with
/// its result.
fn offer_asking(
    host: &Side,
    on_peer: impl FnOnce(&PeerFound) -> bool + Send + 'static,
) -> (PairInvite, HostResult) {
    let (tx, rx) = mpsc::channel();
    let engine = host.engine.clone();
    let join = thread::spawn(move || {
        offer_pairing(
            &engine,
            &Cancel::default(),
            |invite| tx.send(invite).unwrap(),
            on_peer,
        )
    });
    let mut invite = rx.recv().unwrap();
    let port = invite.ticket.addr.unwrap().port();
    invite.ticket = loopback(&invite.ticket, port);
    (invite, join)
}

fn only_record(side: &Side) -> DeviceRecord {
    let known = KnownDevices::open(&*side.platform).unwrap();
    let mut all = known.list(&*side.platform).unwrap();
    assert_eq!(all.len(), 1);
    all.remove(0)
}

#[test]
fn pair_with_invite_remembers_both_sides() {
    let (host, guest) = (Side::named(Some("Host")), Side::named(Some("Guest")));
    let (invite, join) = offer(&host);

    let on_guest = accept_invite(&guest.engine, &invite, &Cancel::default()).unwrap();
    let on_host = join.join().unwrap().unwrap();

    assert_eq!(on_guest.name, "Host");
    assert_eq!(on_host.name, "Guest");
    assert_eq!(only_record(&host), on_host);
    assert_eq!(only_record(&guest), on_guest);
    assert_eq!(on_guest.public, invite.key);
    let policy = PolicyStore::open(&*host.platform).unwrap();
    assert_eq!(
        policy.get(&*host.platform, &on_host.public).unwrap(),
        PolicyDoc::computer_defaults()
    );
}

#[test]
fn pair_with_code_alone_remembers_both_sides() {
    let (host, guest) = (Side::named(Some("Host")), Side::named(Some("Guest")));
    let (invite, join) = offer(&host);

    let on_guest = link_with_code(&guest.engine, &invite.ticket, &Cancel::default()).unwrap();
    join.join().unwrap().unwrap();

    assert_eq!(on_guest.public, invite.key);
    assert_eq!(only_record(&guest), on_guest);
}

#[test]
fn pair_with_wrong_key_fails_key_mismatch() {
    let (host, guest) = (Side::new(), Side::new());
    let (mut invite, join) = offer(&host);
    invite.key = DeviceKey::generate().unwrap().public();

    let err = accept_invite(&guest.engine, &invite, &Cancel::default()).unwrap_err();

    assert!(matches!(err, CoreError::KeyMismatch), "{err:?}");
    assert!(join.join().unwrap().is_err());
    for side in [&host, &guest] {
        let known = KnownDevices::open(&*side.platform).unwrap();
        assert!(known.list(&*side.platform).unwrap().is_empty());
    }
}

#[test]
fn pairing_ignores_the_remember_devices_setting() {
    let host = Side::new();
    let guest = Side::new();
    let guest_engine = guest.engine.clone().with_remember_devices(false);
    let (invite, join) = offer(&host);

    accept_invite(&guest_engine, &invite, &Cancel::default()).unwrap();
    join.join().unwrap().unwrap();

    only_record(&guest);
}

#[test]
fn re_pairing_keeps_the_policy_and_returns_the_stored_record() {
    let (host, guest) = (Side::named(Some("Host")), Side::named(Some("Guest")));
    let (invite, join) = offer(&host);
    let first = accept_invite(&guest.engine, &invite, &Cancel::default()).unwrap();
    join.join().unwrap().unwrap();
    let policy = PolicyStore::open(&*guest.platform).unwrap();
    let set = policy
        .set(
            &*guest.platform,
            &first.public,
            Feature::Clipboard,
            FeatureState::On,
        )
        .unwrap();

    let (invite, join) = offer(&host);
    let again = accept_invite(&guest.engine, &invite, &Cancel::default()).unwrap();
    join.join().unwrap().unwrap();

    assert_eq!(policy.get(&*guest.platform, &again.public).unwrap(), set);
    assert_eq!(again.first_seen, first.first_seen);
    assert_eq!(only_record(&guest), again);
}

#[test]
fn android_peer_starts_on_phone_defaults() {
    let host = Side::named(Some("Host"));
    let phone = Arc::new(
        FakePlatform::with_home(host.tmp.path().join("phone"))
            .with_state_dir(host.tmp.path().join("phone-state"))
            .with_os(OsKind::Android),
    );
    let phone_engine = Engine::new(phone, Vec::new(), Arc::new(NullSink));
    let (invite, join) = offer(&host);

    accept_invite(&phone_engine, &invite, &Cancel::default()).unwrap();
    let on_host = join.join().unwrap().unwrap();

    assert_eq!(on_host.os, OsKind::Android);
    let policy = PolicyStore::open(&*host.platform).unwrap();
    assert_eq!(
        policy.get(&*host.platform, &on_host.public).unwrap(),
        PolicyDoc::phone_defaults()
    );
}

#[test]
fn cancel_ends_offer_pairing_promptly() {
    let host = Side::new();
    let cancel = Cancel::default();
    let (tx, rx) = mpsc::channel();
    let (engine, c) = (host.engine.clone(), cancel.clone());
    let join = thread::spawn(move || offer_pairing(&engine, &c, |i| tx.send(i).unwrap(), |_| true));
    rx.recv().unwrap();

    cancel.cancel();

    let started = std::time::Instant::now();
    assert!(matches!(join.join().unwrap(), Err(CoreError::Cancelled)));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

fn device_key(side: &Side) -> carrova_crypto::identity::DevicePublic {
    let known = KnownDevices::open(&*side.platform).unwrap();
    known.device_key(&*side.platform).unwrap().public()
}

fn assert_nothing_stored(sides: [&Side; 2]) {
    for side in sides {
        let known = KnownDevices::open(&*side.platform).unwrap();
        assert!(known.list(&*side.platform).unwrap().is_empty());
    }
}

#[test]
fn offer_pairing_asks_before_storing() {
    let (host, guest) = (
        Side::named(Some("Host")),
        Side::named(Some("Guest\u{202e}")),
    );
    let (found_tx, found_rx) = mpsc::channel();
    let host_platform = Arc::clone(&host.platform);
    let (invite, join) = offer_asking(&host, move |found| {
        let known = KnownDevices::open(&*host_platform).unwrap();
        let stored_yet = !known.list(&*host_platform).unwrap().is_empty();
        found_tx.send((found.clone(), stored_yet)).unwrap();
        true
    });

    accept_invite(&guest.engine, &invite, &Cancel::default()).unwrap();
    let on_host = join.join().unwrap().unwrap();

    let (found, stored_yet) = found_rx.recv().unwrap();
    assert!(!stored_yet, "asked only before storing");
    assert_eq!(found.name(), "Guest");
    assert_eq!(found.os(), OsKind::Linux);
    assert_eq!(found.public(), device_key(&guest));
    assert_eq!(found.fingerprint(), Fingerprint::of(&found.public()));
    assert_eq!(on_host.public, found.public());
}

#[test]
fn offer_pairing_rejected_peer_stores_nothing_and_joiner_fails() {
    let (host, guest) = (Side::new(), Side::new());
    let (invite, join) = offer_asking(&host, |_| false);

    let joined = accept_invite(&guest.engine, &invite, &Cancel::default());
    let hosted = join.join().unwrap();

    assert!(matches!(hosted, Err(CoreError::PairRejected)), "{hosted:?}");
    assert!(
        matches!(
            joined,
            Err(CoreError::Transport(TransportError::PairingFailed))
        ),
        "{joined:?}"
    );
    assert_nothing_stored([&host, &guest]);
}

#[test]
fn offer_pairing_unanswered_times_out_as_reject() {
    let (host, guest) = (Side::new(), Side::new());
    let (invite, join) = offer_asking(&host, |_| {
        thread::sleep(PAIR_CONFIRM_WINDOW + std::time::Duration::from_secs(1));
        true
    });

    let joined = accept_invite(&guest.engine, &invite, &Cancel::default());
    let hosted = join.join().unwrap();

    assert!(matches!(hosted, Err(CoreError::PairRejected)), "{hosted:?}");
    assert!(joined.is_err(), "{joined:?}");
    assert_nothing_stored([&host, &guest]);
}

#[test]
fn pair_confirm_window_leaves_a_margin_before_the_joiner_gives_up() {
    assert_eq!(
        PAIR_CONFIRM_WINDOW + std::time::Duration::from_secs(10),
        carrova_transport::PAIR_CONFIRM_TIMEOUT
    );
}
