//! Linked devices over the bridge: each operation reaches the link
//! supervisor, here a hermetic one that sees nobody on the network.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, mpsc};
use std::thread;
use std::time::Duration;

use carrova_core::NullSink;
use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::ecosystem::{
    AskId, ClipSettings, PairInvite, PeerSource, PeerState, PolicyStore, SeenPeer,
    SharedSupervisor, SupervisorConfig,
};
use carrova_core::{CoreError, Engine, Ticket};
use carrova_crypto::identity::{DeviceKey, DevicePublic, Fingerprint};
use carrova_crypto::pairing::KnownSecret;
use carrova_link::{Feature, FeatureState};
use carrova_platform::OsKind;
use carrova_view::ipc::ErrorCode;
use tauri::async_runtime::block_on;

use crate::command_error::CommandError;
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
fn ops_send_text_reaches_the_supervisor() {
    let machine = Machine::new();
    remember(&machine, 1, "Pixel 8");
    let state = served(&machine);

    let sent = block_on(ecosystem::send_text(&state, public(1), "hi".to_owned()));

    assert!(sent.is_err(), "not linked");
}

#[test]
fn ops_stop_ring_reaches_the_supervisor() {
    let machine = Machine::new();
    remember(&machine, 1, "Pixel 8");
    let state = served(&machine);

    // Known but not linked: the supervisor's own refusal comes back.
    let stopped = block_on(ecosystem::stop_ring(&state, public(1)));

    assert!(stopped.is_err());
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

#[test]
fn answer_pair_peer_without_question_errors() {
    let machine = Machine::new();

    assert!(ecosystem::answer_pair_peer(&machine.state, true).is_err());
}

/// `host` invites with `pair_invite`, answering its key check with
/// `accept`; `guest` links from the invite's link (`link_invite`).
/// Returns both results and what `host` was shown.
fn pair(
    host: &Machine,
    guest: &Machine,
    accept: bool,
) -> (
    Result<(), CommandError>,
    Result<(), CommandError>,
    serde_json::Value,
) {
    let (uri_tx, uri_rx) = mpsc::channel();
    let answers = Arc::clone(&host.state.peer_answer);
    host.events
        .on_event(move |event| match event["kind"].as_str() {
            Some("pair_invite") => {
                let uri = event["invite"]["uri"].as_str().unwrap().to_owned();
                uri_tx.send(uri).unwrap();
            }
            Some("pair_peer_found") => answers.answer(accept).unwrap(),
            _ => {}
        });
    let state = host.state.clone();
    let hosting = thread::spawn(move || block_on(ecosystem::pair_invite(&state)));
    let shown: PairInvite = uri_rx
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .parse()
        .unwrap();
    let port = shown.ticket.addr.unwrap().port();
    let local = PairInvite {
        ticket: Ticket {
            code: shown.ticket.code.clone(),
            addr: Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)),
        },
        key: shown.key,
    };
    let joined = block_on(ecosystem::link_invite(&guest.state, local.to_uri()));
    let hosted = hosting.join().unwrap();
    let found = host.events.recorded.of_kind("pair_peer_found");
    (hosted, joined, found.first().cloned().unwrap_or_default())
}

fn known(machine: &Machine) -> Vec<DeviceRecord> {
    KnownDevices::open(&machine.platform)
        .unwrap()
        .list(&machine.platform)
        .unwrap()
}

#[test]
fn pair_invite_links_only_after_the_key_check() {
    let (host, guest) = (Machine::new(), Machine::new());

    let (hosted, joined, found) = pair(&host, &guest, true);

    hosted.unwrap();
    joined.unwrap();
    let guest_fingerprint = block_on(ecosystem::own_fingerprint(&guest.state)).unwrap();
    assert_eq!(found["peer"]["fingerprint"], guest_fingerprint.as_str());
    assert_eq!(known(&host).len(), 1);
    assert_eq!(known(&guest).len(), 1);
}

#[test]
fn pair_invite_not_my_device_links_nothing() {
    let (host, guest) = (Machine::new(), Machine::new());

    let (hosted, joined, found) = pair(&host, &guest, false);

    assert!(hosted.is_err() && joined.is_err(), "{hosted:?} {joined:?}");
    assert!(found.is_object());
    assert!(known(&host).is_empty());
    assert!(known(&guest).is_empty());
}

#[test]
fn link_invite_rejects_a_non_invite() {
    let machine = Machine::new();

    let linked = block_on(ecosystem::link_invite(
        &machine.state,
        "7-acid-lemon".into(),
    ));

    assert_eq!(linked.unwrap_err().bridge().code, ErrorCode::BadCode);
}

#[test]
fn own_fingerprint_is_this_devices_key() {
    let machine = Machine::new();

    let shown = block_on(ecosystem::own_fingerprint(&machine.state)).unwrap();

    let key = KnownDevices::open(&machine.platform)
        .unwrap()
        .device_key(&machine.platform)
        .unwrap();
    assert_eq!(shown, Fingerprint::of(&key.public()).to_string());
}
