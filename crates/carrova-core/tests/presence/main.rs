//! Public API of `carrova_core::presence`: what this device announces for
//! each visibility, and the running [`Presence`] as other devices see it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::lan::{NearbyDevice, browse_nearby};
use carrova_core::presence::{Presence, Visibility, advert};
use carrova_core::time::now_unix;
use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::{KnownSecret, TagNonce};
use carrova_platform::{OsKind, Platform};
use carrova_transport::{MAX_KNOWN_TAGS, PresenceAdvert};
use lan_support::{Side, loopback_broadcast_works};

const NOW: u64 = 1_800_000_000;

fn public(seed: u8) -> DevicePublic {
    DeviceKey::from_bytes(&[seed; 32]).public()
}

/// A secret no other test process shares.
fn fresh_secret() -> KnownSecret {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&TagNonce::random().unwrap().0);
    KnownSecret::from_bytes(bytes)
}

/// A device name no other test process uses.
fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", TagNonce::random().unwrap())
}

fn remember(side: &Side, public: DevicePublic, name: &str, last_move: Option<u64>) -> KnownSecret {
    let secret = fresh_secret();
    let record = DeviceRecord {
        public,
        name: name.into(),
        os: OsKind::Windows,
        first_seen: 100,
        last_move,
        moves: 0,
        mac_hint: None,
    };
    KnownDevices::open(&*side.platform)
        .unwrap()
        .remember(&*side.platform, record, &secret)
        .unwrap();
    secret
}

fn advert_at(side: &Side, visibility: Visibility, now: u64) -> Option<PresenceAdvert> {
    advert(&*side.platform, visibility, now).unwrap()
}

fn tagged_with(advert: &PresenceAdvert, secret: &KnownSecret) -> bool {
    advert.known.as_ref().is_some_and(|k| k.made_with(secret))
}

#[test]
fn no_one_advertises_nothing() {
    let side = Side::named(Some("desk"));
    remember(&side, public(1), "laptop", None);
    assert_eq!(advert_at(&side, Visibility::NoOne, NOW), None);
}

#[test]
fn known_mode_hides_name_from_strangers() {
    let side = Side::named(Some("desk"));
    assert_eq!(
        advert_at(&side, Visibility::Known, NOW),
        None,
        "nobody could recognise it: no advert at all"
    );
    let secret = remember(&side, public(1), "laptop", None);
    let advert = advert_at(&side, Visibility::Known, NOW).unwrap();
    assert!(tagged_with(&advert, &secret));
    assert_eq!((advert.name, advert.os), (None, None));
}

#[test]
fn everyone_shows_name_and_os_beside_the_tags() {
    let side = Side::named(Some("desk\u{202e}"));
    let secret = remember(&side, public(1), "laptop", None);
    let until = NOW + 600;
    let advert = advert_at(&side, Visibility::Everyone { until }, NOW).unwrap();
    assert!(tagged_with(&advert, &secret));
    assert_eq!(advert.name.as_deref(), Some("desk"), "sanitised");
    assert_eq!(advert.os, Some(side.platform.os()));
    let alone = Side::named(Some("alone"));
    let advert = advert_at(&alone, Visibility::Everyone { until }, NOW).unwrap();
    assert_eq!(
        (advert.known, advert.name.as_deref()),
        (None, Some("alone"))
    );
}

#[test]
fn everyone_reverts_after_deadline() {
    let side = Side::named(Some("desk"));
    remember(&side, public(1), "laptop", None);
    let until = NOW + 600;
    let everyone = Visibility::Everyone { until };
    assert_eq!(everyone.at(until - 1), everyone);
    assert_eq!(everyone.at(until), Visibility::Known);
    assert!(
        advert_at(&side, everyone, until - 1)
            .unwrap()
            .name
            .is_some()
    );
    let reverted = advert_at(&side, everyone, until).unwrap();
    assert_eq!((reverted.name, reverted.os), (None, None));
}

#[test]
fn everyone_already_past_deadline_on_start_is_known() {
    if !loopback_broadcast_works() {
        return;
    }
    assert_eq!(
        advert_at(
            &Side::named(Some("desk")),
            Visibility::Everyone { until: NOW - 1 },
            NOW
        ),
        None,
        "no known devices and past its deadline: nothing to announce"
    );
    let name = unique("past");
    let side = Side::named(Some(&name));
    remember(&side, public(1), "laptop", None);
    let presence = Presence::start(
        &side.engine,
        Visibility::Everyone {
            until: now_unix() - 1,
        },
    );
    let stranger = Side::new();
    let seen = names(&browse_nearby(&*stranger.platform).unwrap());
    assert!(!seen.contains(&name), "{seen:?}");
    presence.stop();
}

#[test]
fn kt_caps_at_13_most_recent() {
    let side = Side::named(Some("desk"));
    let count = u8::try_from(MAX_KNOWN_TAGS).unwrap() + 3;
    // Seed i moved last at time i; seed 0 never moved but was seen last.
    let secrets: Vec<_> = (0..count)
        .map(|i| {
            remember(
                &side,
                public(i + 1),
                "peer",
                (i > 0).then_some(u64::from(i)),
            )
        })
        .collect();
    let advert = advert_at(&side, Visibility::Known, NOW).unwrap();
    let tagged: Vec<usize> = (0..secrets.len())
        .filter(|&i| tagged_with(&advert, &secrets[i]))
        .collect();
    let newest: Vec<usize> = (secrets.len() - MAX_KNOWN_TAGS..secrets.len()).collect();
    assert_eq!(tagged, newest);
}

fn names(found: &[NearbyDevice]) -> Vec<String> {
    found.iter().map(|d| d.name.clone()).collect()
}

#[test]
fn a_known_peer_finds_a_present_device_by_its_stored_name_and_strangers_do_not() {
    if !loopback_broadcast_works() {
        return;
    }
    let desk = Side::named(Some("desk-advertised"));
    let laptop = Side::new();
    let desk_id = public(7);
    let stored = unique("my desk");
    let secret = remember(&desk, public(8), "laptop", Some(5));
    // The laptop remembers the desk with the same shared secret.
    let record = DeviceRecord {
        public: desk_id,
        name: stored.clone(),
        os: OsKind::Linux,
        first_seen: 1,
        last_move: None,
        moves: 1,
        mac_hint: None,
    };
    KnownDevices::open(&*laptop.platform)
        .unwrap()
        .remember(&*laptop.platform, record, &secret)
        .unwrap();
    let presence = Presence::start(&desk.engine, Visibility::Known);

    let found = browse_nearby(&*laptop.platform).unwrap();
    let desk_seen: Vec<_> = found.iter().filter(|d| d.known == Some(desk_id)).collect();
    assert_eq!(desk_seen.len(), 1, "{found:?}");
    assert_eq!(
        desk_seen[0].name, stored,
        "name from the store, not the advert"
    );
    assert_eq!(desk_seen[0].os, Some(OsKind::Linux));
    assert!(desk_seen[0].answered_ms <= 2_500);

    let stranger = Side::new();
    let seen = browse_nearby(&*stranger.platform).unwrap();
    assert!(
        seen.iter().all(|d| d.known.is_none() && d.name != stored),
        "{seen:?}"
    );
    presence.stop();
}

#[test]
fn a_running_presence_stops_naming_itself_when_everyone_ends() {
    if !loopback_broadcast_works() {
        return;
    }
    let name = unique("everyone");
    let side = Side::named(Some(&name));
    let until = now_unix() + 3;
    let presence = Presence::start(&side.engine, Visibility::Everyone { until });
    let stranger = Side::new();
    let seen = names(&browse_nearby(&*stranger.platform).unwrap());
    assert!(
        seen.contains(&name),
        "named while everyone may see it: {seen:?}"
    );
    while now_unix() <= until {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let seen = names(&browse_nearby(&*stranger.platform).unwrap());
    assert!(!seen.contains(&name), "reverted to known devices: {seen:?}");
    presence.set(Visibility::Everyone {
        until: now_unix() + 60,
    });
    let seen = names(&browse_nearby(&*stranger.platform).unwrap());
    assert!(seen.contains(&name), "shown again when set: {seen:?}");
    presence.stop();
}
