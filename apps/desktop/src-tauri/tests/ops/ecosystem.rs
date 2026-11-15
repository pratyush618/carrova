//! Linked devices over the bridge: each operation reaches the link
//! supervisor, here a hermetic one that sees nobody on the network.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use carrova_core::NullSink;
use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::ecosystem::{
    AskId, ClipSettings, PeerSource, PeerState, PolicyStore, SeenPeer, SharedSupervisor,
    SupervisorConfig,
};
use carrova_core::{CoreError, Engine};
use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::KnownSecret;
use carrova_link::{Feature, FeatureState};
use carrova_platform::OsKind;
use carrova_view::ipc::ErrorCode;
use tauri::async_runtime::block_on;

use crate::ops::ecosystem;
use crate::state::AppState;
use crate::testing::Machine;

/// A network where no known device is present.
struct Nobody;

impl PeerSource for Nobody {
    fn seen(&self) -> Vec<SeenPeer> {
        Vec::new()
    }
}

fn public(seed: u8) -> DevicePublic {
    DeviceKey::from_bytes(&[seed; 32]).public()
}

fn remember(machine: &Machine, seed: u8, name: &str) {
    let record = DeviceRecord {
        public: public(seed),
        name: name.into(),
        os: OsKind::Android,
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

/// A link supervisor on `machine` that sees nobody on the network.
fn hermetic(machine: &Machine) -> Result<SharedSupervisor, CoreError> {
    let engine = Engine::new(
        Arc::new(machine.platform.clone()),
        Vec::new(),
        Arc::new(NullSink),
    );
    SharedSupervisor::attach_with(engine, Arc::new(Nobody), SupervisorConfig::default())
}

/// `machine`'s state with its link supervisor serving it.
fn served(machine: &Machine) -> AppState {
    ecosystem::serve_link(&machine.state, || hermetic(machine)).unwrap();
    machine.state.clone()
}

#[test]
fn ops_set_feature_persists() {
    let machine = Machine::new();
    remember(&machine, 1, "Pixel 8");
    let state = served(&machine);

    block_on(ecosystem::set_feature(
        &state,
        public(1),
        Feature::Ring,
        FeatureState::On,
    ))
    .unwrap();

    let stored = PolicyStore::open(&machine.platform)
        .unwrap()
        .get(&machine.platform, &public(1))
        .unwrap();
    assert_eq!(stored.get(Feature::Ring), FeatureState::On);
}

#[test]
fn ops_answer_unknown_ask_errors() {
    let machine = Machine::new();
    let state = served(&machine);
    let unknown: AskId = serde_json::from_value(serde_json::json!(42)).unwrap();

    let answered = block_on(ecosystem::answer_ask(&state, unknown, true));

    assert!(answered.is_err());
}

#[test]
fn ops_linked_devices_maps_views() {
    let machine = Machine::new();
    remember(&machine, 1, "Pixel 8");
    let state = served(&machine);

    let views = block_on(ecosystem::linked_devices(&state)).unwrap();

    assert_eq!(views.len(), 1);
    assert_eq!(views[0].id, public(1).to_hex());
    assert_eq!(views[0].name, "Pixel 8");
    assert_eq!(views[0].os, OsKind::Android);
    assert_ne!(views[0].state, PeerState::Linked);
}

#[test]
fn ops_clip_settings_round_trip() {
    let machine = Machine::new();
    let state = served(&machine);
    let quiet = ClipSettings {
        share: false,
        show_note: true,
        history_file: false,
    };

    block_on(ecosystem::set_clip_settings(&state, quiet)).unwrap();

    assert_eq!(block_on(ecosystem::clip_settings(&state)).unwrap(), quiet);
}

#[test]
fn ops_retry_of_an_unknown_file_errors() {
    let machine = Machine::new();
    let state = served(&machine);

    let retried = block_on(ecosystem::retry_file(&state, 42));

    assert_eq!(retried.unwrap_err().bridge().code, ErrorCode::KnownDevice);
}

#[test]
fn start_once_starts_one_supervisor_for_racing_starters() {
    let machine = Machine::new();
    let slot = OnceLock::new();
    let starts = AtomicUsize::new(0);
    let start = || {
        starts.fetch_add(1, Ordering::SeqCst);
        hermetic(&machine)
    };

    std::thread::scope(|s| {
        let racers: Vec<_> = (0..4)
            .map(|_| s.spawn(|| ecosystem::start_once(&slot, start).map(drop)))
            .collect();
        for racer in racers {
            racer.join().unwrap().unwrap();
        }
    });

    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert!(slot.get().is_some());
}

#[test]
fn start_once_tries_again_after_a_failed_start() {
    let machine = Machine::new();
    let slot = OnceLock::new();

    let failed = ecosystem::start_once(&slot, || Err(CoreError::NotLinked));
    assert!(failed.is_err());
    assert!(slot.get().is_none());

    ecosystem::start_once(&slot, || hermetic(&machine)).unwrap();
    assert!(slot.get().is_some());
}

#[test]
fn serving_keeps_the_supervisor_the_link_service_started() {
    let machine = Machine::new();
    remember(&machine, 1, "Pixel 8");
    // The phone's link service started it before the app's window existed.
    ecosystem::start_once(&machine.state.link, || hermetic(&machine)).unwrap();

    let mut started_again = false;
    ecosystem::serve_link(&machine.state, || {
        started_again = true;
        hermetic(&machine)
    })
    .unwrap();

    assert!(!started_again);
    let views = block_on(ecosystem::linked_devices(&machine.state)).unwrap();
    assert_eq!(views.len(), 1);
}

#[test]
fn the_process_has_one_link_slot() {
    assert!(Arc::ptr_eq(
        &ecosystem::process_link(),
        &ecosystem::process_link()
    ));
}
