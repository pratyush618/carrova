//! Linked devices over the bridge: each operation reaches the link
//! supervisor, here a hermetic one that sees nobody on the network.

use std::sync::Arc;

use carrova_core::Engine;
use carrova_core::NullSink;
use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::ecosystem::{
    AskId, PeerSource, PeerState, PolicyStore, SeenPeer, SharedSupervisor, SupervisorConfig,
};
use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::KnownSecret;
use carrova_link::{Feature, FeatureState};
use carrova_platform::OsKind;
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

/// `machine`'s state with its link supervisor serving it.
fn served(machine: &Machine) -> AppState {
    let engine = Engine::new(
        Arc::new(machine.platform.clone()),
        Vec::new(),
        Arc::new(NullSink),
    );
    let link = SharedSupervisor::attach_with(engine, Arc::new(Nobody), SupervisorConfig::default())
        .unwrap();
    ecosystem::serve_link(&machine.state, link).unwrap();
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
