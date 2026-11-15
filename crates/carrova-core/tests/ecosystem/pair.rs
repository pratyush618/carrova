//! Pair-only linking: invite (pinned) and typed code (unpinned).

#![allow(clippy::unwrap_used)]

use std::sync::mpsc;
use std::thread;

use carrova_core::CoreError;
use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::ecosystem::{
    PairInvite, PolicyStore, accept_invite, link_with_code, offer_pairing,
};
use carrova_crypto::identity::DeviceKey;
use carrova_link::PolicyDoc;

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
    let join = thread::spawn(move || offer_pairing(&engine, |invite| tx.send(invite).unwrap()));
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

    let on_guest = accept_invite(&guest.engine, &invite).unwrap();
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

    let on_guest = link_with_code(&guest.engine, &invite.ticket).unwrap();
    join.join().unwrap().unwrap();

    assert_eq!(on_guest.public, invite.key);
    assert_eq!(only_record(&guest), on_guest);
}

#[test]
fn pair_with_wrong_key_fails_key_mismatch() {
    let (host, guest) = (Side::new(), Side::new());
    let (mut invite, join) = offer(&host);
    invite.key = DeviceKey::generate().unwrap().public();

    let err = accept_invite(&guest.engine, &invite).unwrap_err();

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

    accept_invite(&guest_engine, &invite).unwrap();
    join.join().unwrap().unwrap();

    only_record(&guest);
}
