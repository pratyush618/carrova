//! The Phase 4 contract additions, driven over the bridge state on fake
//! machines: about, rollback preview, transfer controls and sending again
//! after a cancel.
#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use carrova_core::{KdfStrength, PlanOptions, SessionId};
use carrova_view::ipc::ErrorCode;
use tauri::async_runtime::{block_on, spawn};

use crate::ops::{about, receive, send, sessions};
use crate::testing::{Machine, PASS};

/// A sender with the app's config, scanned (the engine holds the
/// suggested selection).
fn selected_sender() -> (Machine, SessionId) {
    let sender = Machine::new();
    sender.put(".config/app/settings.toml", b"theme = 'dark'\n");
    let scan = block_on(send::scan(&sender.state)).unwrap();
    (sender, scan.result().session)
}

/// A receiver that applied the sender's archive: it and the import id.
fn applied_import() -> (Machine, SessionId) {
    let (sender, id) = selected_sender();
    let archive = sender.scratch("move.carrova");
    let strength = KdfStrength::Strong;
    block_on(send::export_archive(
        &sender.state,
        &id,
        archive.clone(),
        PASS.into(),
        strength,
    ))
    .unwrap();
    let receiver = Machine::new();
    let opened = block_on(receive::open_archive(&receiver.state, archive, PASS.into())).unwrap();
    block_on(receive::plan(
        &receiver.state,
        &opened.session,
        PlanOptions::default(),
    ))
    .unwrap();
    block_on(receive::approve(&receiver.state, &opened.session)).unwrap();
    block_on(receive::apply(&receiver.state, &opened.session)).unwrap();
    (receiver, opened.session)
}

/// Wait until `machine` has emitted `n` pairing codes.
fn wait_for_codes(machine: &Machine, n: usize) {
    let until = Instant::now() + Duration::from_secs(20);
    while machine.events.recorded.of_kind("pairing_code").len() < n {
        assert!(Instant::now() < until, "no pairing code #{n}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn about_names_the_versions_and_the_os() {
    let machine = Machine::new();
    let about = about::about(&machine.state);
    assert_eq!(about.app_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(about.engine_version, carrova_core::about::ENGINE_VERSION);
    assert_eq!(about.os, carrova_platform::OsKind::Linux);
}

#[test]
fn a_rollback_preview_counts_without_undoing() {
    let (receiver, id) = applied_import();
    let file = receiver.home.join(".config/app/settings.toml");

    let preview = block_on(sessions::rollback_preview(
        &receiver.state,
        Some(id.clone()),
    ))
    .unwrap();

    assert_eq!(
        preview.removed, 2,
        "the file and its new folder: {preview:?}"
    );
    assert!(preview.blockers.is_empty());
    assert!(file.exists(), "nothing was undone");
    let record = block_on(sessions::list(&receiver.state)).unwrap();
    assert_eq!(record[0].state, carrova_core::State::Done);
}

#[test]
fn transfer_controls_reach_only_live_imports() {
    let (receiver, id) = applied_import();
    receive::set_transfer_paused(&receiver.state, &id, true).unwrap();
    receive::set_transfer_paused(&receiver.state, &id, false).unwrap();
    receive::ignore_free_space(&receiver.state, &id).unwrap();

    let unknown = SessionId("20261108-000000-000000".into());
    let errors = [
        receive::set_transfer_paused(&receiver.state, &unknown, true).unwrap_err(),
        receive::ignore_free_space(&receiver.state, &unknown).unwrap_err(),
    ];
    for err in errors {
        assert_eq!(err.bridge().code, ErrorCode::Session);
    }
}

#[test]
fn a_cancelled_send_runs_again_with_a_new_code() {
    let (sender, id) = selected_sender();
    let state = sender.state.clone();
    let session = id.clone();
    let first = spawn(async move { send::run_send(&state, &session).await });
    wait_for_codes(&sender, 1);
    sessions::cancel(&sender.state, &id).unwrap();
    let err = block_on(first).unwrap().unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Cancelled);

    let state = sender.state.clone();
    let session = id.clone();
    let again = spawn(async move { send::run_send(&state, &session).await });
    wait_for_codes(&sender, 2);
    let codes = sender.events.recorded.of_kind("pairing_code");
    assert_ne!(codes[0]["code"], codes[1]["code"], "a fresh code");
    sessions::cancel(&sender.state, &id).unwrap();
    assert!(block_on(again).unwrap().is_err());
}
