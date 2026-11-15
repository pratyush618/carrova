//! Pair-only linking: invite (pinned) and typed code (unpinned).

#![allow(clippy::unwrap_used)]

use std::sync::{Arc, mpsc};
use std::thread;

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::ecosystem::{
    PairInvite, PolicyStore, accept_invite, link_with_code, offer_pairing,
};
use carrova_core::{Cancel, CoreError, Engine, NullSink};
use carrova_crypto::identity::DeviceKey;
use carrova_link::{Feature, FeatureState, PolicyDoc};
use carrova_platform::OsKind;
use carrova_platform::fake::FakePlatform;

use crate::lan_support::{Side, loopback};

/// `host` offers a pairing; returns its invite (pointed at loopback) and
/// the thread that ends with its result.
fn offer(
    host: &Side,
) -> (
    PairInvite,
    thread::JoinHandle<carrova_core::Result<DeviceRecord>>,
) {
    let (tx, rx) = mpsc::channel();
    let engine = host.engine.clone();
    let join = thread::spawn(move || {
        offer_pairing(&engine, &Cancel::default(), |invite| {
            tx.send(invite).unwrap();
        })
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
    let join = thread::spawn(move || offer_pairing(&engine, &c, |i| tx.send(i).unwrap()));
    rx.recv().unwrap();

    cancel.cancel();

    let started = std::time::Instant::now();
    assert!(matches!(join.join().unwrap(), Err(CoreError::Cancelled)));
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}
