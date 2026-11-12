//! Public API of `carrova_core::presence`: what this device announces for
//! each visibility, and the running [`Presence`] as other devices see it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
mod follow;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;
mod visibility;

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::lan::{NearbyDevice, browse_nearby};
use carrova_core::presence::{Presence, Visibility, advert, save_visibility};
use carrova_core::time::now_unix;
use carrova_core::{Importer, PlanOptions};
use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::{KnownSecret, TagNonce};
use carrova_platform::{OsKind, Platform};
use carrova_transport::{MAX_KNOWN_TAGS, PresenceAdvert};
use lan_support::{Side, loopback_broadcast_works, run};

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
    remember_with(side, public, name, last_move, &secret);
    secret
}

/// `side` remembers the device `public` as `name`, sharing `secret`.
fn remember_with(
    side: &Side,
    public: DevicePublic,
    name: &str,
    last_move: Option<u64>,
    secret: &KnownSecret,
) {
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
        .remember(&*side.platform, record, secret)
        .unwrap();
}

fn advert_at(side: &Side, visibility: Visibility, now: u64) -> Option<PresenceAdvert> {
    advert(&*side.platform, visibility, now).unwrap()
}

/// Whether `side` made one of `advert`'s tags for the peer it shares
/// `secret` with.
fn tagged_with(side: &Side, advert: &PresenceAdvert, secret: &KnownSecret) -> bool {
    let me = own_key(side);
    advert
        .known
        .as_ref()
        .is_some_and(|k| k.made_with(secret, &me))
}

/// `side`'s own identity key.
fn own_key(side: &Side) -> DevicePublic {
    KnownDevices::open(&*side.platform)
        .unwrap()
        .device_key(&*side.platform)
        .unwrap()
        .public()
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
    assert!(tagged_with(&side, &advert, &secret));
    assert_eq!((advert.name, advert.os), (None, None));
}

#[test]
fn everyone_shows_name_and_os_beside_the_tags() {
    let side = Side::named(Some("desk\u{202e}"));
    let secret = remember(&side, public(1), "laptop", None);
    let until = NOW + 600;
    let advert = advert_at(&side, Visibility::Everyone { until }, NOW).unwrap();
    assert!(tagged_with(&side, &advert, &secret));
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
    let past = Visibility::Everyone { until: NOW - 1 };
    assert_eq!(
        advert_at(&Side::named(Some("desk")), past, NOW),
        None,
        "no known devices and past its deadline: nothing to announce"
    );
    let name = unique("past");
    let desk = Side::named(Some(&name));
    let laptop = Side::new();
    let secret = remember(&desk, own_key(&laptop), "laptop", None);
    let advert = advert_at(&desk, past, NOW).unwrap();
    assert!(tagged_with(&desk, &advert, &secret));
    assert_eq!((advert.name, advert.os), (None, None));

    if !loopback_broadcast_works() {
        return;
    }
    remember_with(&laptop, own_key(&desk), "my desk", None, &secret);
    let presence = Presence::start(
        &desk.engine,
        Visibility::Everyone {
            until: now_unix() - 1,
        },
    );
    let found = browse_nearby(&*laptop.platform).unwrap();
    assert!(
        found.iter().any(|d| d.known == Some(own_key(&desk))),
        "known devices still find it by tag: {found:?}"
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
        .filter(|&i| tagged_with(&side, &advert, &secrets[i]))
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
    let desk_id = own_key(&desk);
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

#[test]
fn a_device_does_not_find_itself_by_its_own_tags() {
    if !loopback_broadcast_works() {
        return;
    }
    let desk = Side::new();
    let laptop = public(9);
    remember(&desk, laptop, "laptop", Some(5));
    let presence = Presence::start(&desk.engine, Visibility::Known);
    let found = browse_nearby(&*desk.platform).unwrap();
    assert!(
        found.iter().all(|d| d.known != Some(laptop)),
        "the laptop is off: only the desk's own advert is out there: {found:?}"
    );
    presence.stop();
}

#[test]
fn a_known_sender_offering_a_code_is_one_code_offer_and_one_known_device() {
    if !loopback_broadcast_works() {
        return;
    }
    let name = unique("sender");
    let sender = Side::seeded(Side::named(Some(&name)));
    let receiver = Side::new();
    let secret = remember(&sender, own_key(&receiver), "receiver", Some(1));
    remember_with(&receiver, own_key(&sender), "my sender", None, &secret);
    let until = now_unix() + 60;
    let presence = Presence::start(&sender.engine, Visibility::Everyone { until });
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());

    let found = browse_nearby(&*receiver.platform).unwrap();
    let offers: Vec<_> = found.iter().filter(|d| d.offers_code).collect();
    assert_eq!(offers.len(), 1, "{found:?}");
    assert_eq!(
        (offers[0].name.as_str(), offers[0].known),
        (name.as_str(), None)
    );
    let present: Vec<_> = found.iter().filter(|d| d.known.is_some()).collect();
    assert_eq!(present.len(), 1, "{found:?}");
    assert!(!present[0].offers_code);
    assert_eq!(present[0].name, "my sender");

    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    })
    .unwrap();
    imp.end_dry_run().unwrap();
    running.thread.join().unwrap().unwrap();
    presence.stop();
}

#[test]
fn browsing_creates_no_known_device_store() {
    let side = Side::new();
    let devices = side.platform.state_dir().unwrap().join("devices");
    browse_nearby(&*side.platform).unwrap();
    assert!(!devices.exists(), "a plain browse writes nothing");
}

#[test]
fn a_corrupt_known_device_store_does_not_stop_browsing() {
    let side = Side::new();
    let devices = side.platform.state_dir().unwrap().join("devices");
    std::fs::create_dir_all(&devices).unwrap();
    std::fs::write(devices.join("known.json"), b"{not json").unwrap();
    let found = browse_nearby(&*side.platform).unwrap();
    assert!(found.iter().all(|d| d.known.is_none()), "{found:?}");
}

#[test]
fn an_advert_reads_only_the_secrets_it_tags() {
    let side = Side::named(Some("desk"));
    let oldest = public(1);
    remember(&side, oldest, "oldest", Some(1));
    for i in 0..u8::try_from(MAX_KNOWN_TAGS).unwrap() {
        remember(&side, public(i + 2), "peer", Some(u64::from(i) + 2));
    }
    let key = side
        .platform
        .state_dir()
        .unwrap()
        .join(format!("devices/{}.key", oldest.to_hex()));
    std::fs::write(key, b"short").unwrap();
    assert!(advert(&*side.platform, Visibility::Known, NOW).is_ok());
}

#[test]
fn this_computer_does_not_list_itself_when_shown_to_everyone() {
    if !loopback_broadcast_works() {
        return;
    }
    let name = unique("self");
    let side = Side::named(Some(&name));
    let presence = Presence::start(
        &side.engine,
        Visibility::Everyone {
            until: now_unix() + 60,
        },
    );
    let stranger = Side::new();
    let seen = names(&browse_nearby(&*stranger.platform).unwrap());
    assert!(seen.contains(&name), "others see it: {seen:?}");

    let own = names(&browse_nearby(&*side.platform).unwrap());
    assert!(!own.contains(&name), "{own:?}");
    presence.stop();
}

#[test]
fn a_running_presence_follows_the_saved_visibility() {
    if !loopback_broadcast_works() {
        return;
    }
    let name = unique("follows");
    let side = Side::named(Some(&name));
    let presence = Presence::start(&side.engine, Visibility::NoOne);
    let stranger = Side::new();
    let state = side.platform.state_dir().unwrap();
    // As the platform reports a file saved there (the CLI's).
    let settle = || {
        side.platform.fire_dir_change(&state);
        std::thread::sleep(std::time::Duration::from_millis(300));
    };

    let everyone = Visibility::Everyone {
        until: now_unix() + 60,
    };
    save_visibility(&*side.platform, everyone).unwrap();
    settle();
    let seen = names(&browse_nearby(&*stranger.platform).unwrap());
    assert!(seen.contains(&name), "shown once saved: {seen:?}");

    save_visibility(&*side.platform, Visibility::NoOne).unwrap();
    settle();
    let seen = names(&browse_nearby(&*stranger.platform).unwrap());
    assert!(!seen.contains(&name), "hidden once saved: {seen:?}");
    presence.stop();
}
