//! Nearby and known-device view-models (public API).
#![allow(clippy::unwrap_used)]

use carrova_core::devices::DeviceRecord;
use carrova_core::lan::NearbyDevice;
use carrova_core::presence::Visibility;
use carrova_crypto::identity::{DeviceKey, DevicePublic, Fingerprint};
use carrova_platform::OsKind;
use carrova_view::devices::{
    ForgetDevices, INNER_RING_MS, KnownDeviceView, NearbyDeviceView, Ring, VisibilityChoice,
    VisibilityView, chosen_visibility, known_device_views, nearby_views, ring, visibility_view,
};

const NOW: u64 = 1_800_000_000;

fn public(seed: u8) -> DevicePublic {
    DeviceKey::from_bytes(&[seed; 32]).public()
}

#[test]
fn devices_answering_within_300_ms_sit_on_the_inner_ring() {
    assert_eq!(INNER_RING_MS, 300);
    assert_eq!(ring(0), Ring::Inner);
    assert_eq!(ring(299), Ring::Inner);
    assert_eq!(ring(300), Ring::Outer);
    assert_eq!(ring(u32::MAX), Ring::Outer);
}

#[test]
fn a_known_nearby_device_carries_its_id_and_fingerprint() {
    let found = [
        NearbyDevice {
            name: "Desk".into(),
            os: Some(OsKind::Windows),
            known: Some(public(1)),
            offers_code: false,
            answered_ms: 120,
        },
        NearbyDevice {
            name: "new-mbp".into(),
            os: Some(OsKind::MacOs),
            known: None,
            offers_code: true,
            answered_ms: 800,
        },
    ];
    assert_eq!(
        nearby_views(&found),
        [
            NearbyDeviceView {
                name: "Desk".into(),
                os: Some(OsKind::Windows),
                known: true,
                id: Some(public(1).to_hex()),
                fingerprint: Some(Fingerprint::of(&public(1)).to_string()),
                answered_ms: 120,
                ring: Ring::Inner,
                offers_code: false,
            },
            NearbyDeviceView {
                name: "new-mbp".into(),
                os: Some(OsKind::MacOs),
                known: false,
                id: None,
                fingerprint: None,
                answered_ms: 800,
                ring: Ring::Outer,
                offers_code: true,
            },
        ]
    );
}

#[test]
fn a_known_device_shows_its_history_and_only_whether_it_has_a_wake_hint() {
    let record = DeviceRecord {
        public: public(2),
        name: "Old ThinkPad".into(),
        os: OsKind::Linux,
        first_seen: 10,
        last_move: Some(20),
        moves: 3,
        mac_hint: Some([1, 2, 3, 4, 5, 6]),
    };
    assert_eq!(
        known_device_views(&[record]),
        [KnownDeviceView {
            id: public(2).to_hex(),
            fingerprint: Fingerprint::of(&public(2)).to_string(),
            name: "Old ThinkPad".into(),
            os: OsKind::Linux,
            first_seen: 10,
            last_move: Some(20),
            moves: 3,
            has_wake_hint: true,
        }]
    );
}

#[test]
fn choosing_everyone_sets_the_shared_deadline() {
    assert_eq!(
        chosen_visibility(VisibilityChoice::NoOne, NOW),
        Visibility::NoOne
    );
    assert_eq!(
        chosen_visibility(VisibilityChoice::Known, NOW),
        Visibility::Known
    );
    assert_eq!(
        chosen_visibility(VisibilityChoice::Everyone, NOW),
        Visibility::everyone_from(NOW)
    );
}

#[test]
fn the_visibility_view_names_the_choice_and_its_deadline() {
    assert_eq!(
        visibility_view(Visibility::Everyone { until: NOW }),
        VisibilityView {
            choice: VisibilityChoice::Everyone,
            until: Some(NOW),
        }
    );
    assert_eq!(
        visibility_view(Visibility::Known),
        VisibilityView {
            choice: VisibilityChoice::Known,
            until: None,
        }
    );
}

#[test]
fn forget_takes_all_or_one_device() {
    let all: ForgetDevices = serde_json::from_str(r#""all""#).unwrap();
    assert_eq!(all, ForgetDevices::All);
    let one: ForgetDevices = serde_json::from_str(r#"{"device":"7F3A19C2"}"#).unwrap();
    assert_eq!(one, ForgetDevices::Device("7F3A19C2".into()));
}
