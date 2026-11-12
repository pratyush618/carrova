//! Known devices and visibility: each change reaches the presence advert.

use std::sync::Arc;

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::presence::{Visibility, load_visibility};
use carrova_core::time::now_unix;
use carrova_crypto::identity::{DeviceKey, DevicePublic, Fingerprint};
use carrova_crypto::pairing::KnownSecret;
use carrova_platform::OsKind;
use carrova_view::devices::{ForgetDevices, VisibilityChoice};
use carrova_view::ipc::ErrorCode;
use tauri::async_runtime::block_on;

use crate::ops::devices;
use crate::state::AppState;
use crate::testing::{Machine, Refreshes};

/// `machine`'s state with a presence that counts refreshes.
fn counting(machine: &Machine) -> (AppState, Arc<Refreshes>) {
    let presence = Arc::new(Refreshes::default());
    let state = AppState {
        presence: presence.clone(),
        ..machine.state.clone()
    };
    (state, presence)
}

fn public(seed: u8) -> DevicePublic {
    DeviceKey::from_bytes(&[seed; 32]).public()
}

fn remember(machine: &Machine, seed: u8, name: &str) {
    let record = DeviceRecord {
        public: public(seed),
        name: name.into(),
        os: OsKind::Linux,
        first_seen: 100,
        last_move: None,
        moves: 0,
        mac_hint: None,
    };
    KnownDevices::open(&machine.platform)
        .unwrap()
        .remember(
            &machine.platform,
            record,
            &KnownSecret::from_bytes([seed; 32]),
        )
        .unwrap();
}

fn names(state: &AppState) -> Vec<String> {
    block_on(devices::known_devices(state))
        .unwrap()
        .into_iter()
        .map(|d| d.name)
        .collect()
}

#[test]
fn forget_all_forgets_every_device_and_refreshes_presence() {
    let machine = Machine::new();
    let (state, presence) = counting(&machine);
    remember(&machine, 1, "Desk");
    remember(&machine, 2, "Laptop");

    block_on(devices::forget_device(&state, ForgetDevices::All)).unwrap();

    assert!(names(&state).is_empty());
    assert_eq!(presence.count(), 1);
}

#[test]
fn forget_one_device_by_its_fingerprint() {
    let machine = Machine::new();
    let (state, presence) = counting(&machine);
    remember(&machine, 1, "Desk");
    remember(&machine, 2, "Laptop");
    let fingerprint = Fingerprint::of(&public(1)).to_string();

    block_on(devices::forget_device(
        &state,
        ForgetDevices::Device(fingerprint),
    ))
    .unwrap();

    assert_eq!(names(&state), ["Laptop"]);
    assert_eq!(presence.count(), 1);
}

#[test]
fn rename_device_by_id() {
    let machine = Machine::new();
    let (state, _) = counting(&machine);
    remember(&machine, 1, "Desk");

    block_on(devices::rename_device(
        &state,
        public(1).to_hex(),
        "Office PC".into(),
    ))
    .unwrap();

    assert_eq!(names(&state), ["Office PC"]);
}

#[test]
fn renaming_an_unknown_device_fails() {
    let machine = Machine::new();
    let (state, _) = counting(&machine);
    assert!(
        block_on(devices::rename_device(
            &state,
            public(9).to_hex(),
            "x".into()
        ))
        .is_err()
    );
}

#[test]
fn visibility_starts_at_known_devices() {
    let machine = Machine::new();
    let (state, _) = counting(&machine);
    let view = block_on(devices::visibility(&state)).unwrap();
    assert_eq!((view.choice, view.until), (VisibilityChoice::Known, None));
}

#[test]
fn set_visibility_saves_it_and_reapplies_presence() {
    let machine = Machine::new();
    let (state, presence) = counting(&machine);

    let before = now_unix();
    let view = block_on(devices::set_visibility(&state, VisibilityChoice::Everyone)).unwrap();

    assert_eq!(view.choice, VisibilityChoice::Everyone);
    let until = view.until.unwrap();
    assert!(until >= before + 600, "{until}");
    assert_eq!(
        load_visibility(&machine.platform, before).unwrap(),
        Visibility::Everyone { until }
    );
    assert_eq!(block_on(devices::visibility(&state)).unwrap(), view);
    assert_eq!(presence.count(), 1);
}

#[test]
fn reset_identity_forgets_devices_and_refreshes_presence() {
    let machine = Machine::new();
    let (state, presence) = counting(&machine);
    remember(&machine, 1, "Desk");
    let before = KnownDevices::open(&machine.platform)
        .unwrap()
        .device_key(&machine.platform)
        .unwrap()
        .public();

    block_on(devices::reset_identity(&state)).unwrap();

    assert!(names(&state).is_empty());
    let after = KnownDevices::open(&machine.platform)
        .unwrap()
        .device_key(&machine.platform)
        .unwrap()
        .public();
    assert_ne!(before, after);
    assert_eq!(presence.count(), 1);
}

#[test]
fn waking_a_device_without_a_wake_hint_says_so() {
    let machine = Machine::new();
    let (state, _) = counting(&machine);
    remember(&machine, 1, "Desk");

    let error = block_on(devices::wake_device(&state, public(1).to_hex())).unwrap_err();

    assert_eq!(error.bridge().code, ErrorCode::KnownDevice);
    assert!(error.bridge().message.contains("wake hint"), "{error}");
}
