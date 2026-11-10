//! Archive migration through the bridge: scan → select → export on one
//! machine, open → plan → approve → apply on another.

use carrova_core::{KdfStrength, PlanApproval, PlanOptions, SessionId};
use carrova_platform::OsKind;
use carrova_view::ipc::{ErrorCode, SourceDevice};
use serde_json::Value;
use tauri::async_runtime::block_on;

use crate::ops::{receive, send, sessions};
use crate::testing::{Machine, PASS};

/// A sender with the app's config, scanned: the engine holds the
/// suggested selection.
fn selected_sender() -> (Machine, SessionId) {
    let sender = Machine::new();
    sender.put(".config/app/settings.toml", b"theme = 'dark'\n");
    let scan = block_on(send::scan(&sender.state)).unwrap();
    assert!(scan.result().view.issues.is_empty());
    (sender, scan.result().session)
}

/// Engine event names `machine` emitted for `id`, in order.
fn engine_events(machine: &Machine, id: &SessionId) -> Vec<String> {
    machine
        .events
        .recorded
        .of_kind("engine")
        .iter()
        .filter(|e| e["session"] == id.0.as_str())
        .map(|e| e["event"]["event"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn an_archive_round_trip_moves_the_selected_files() {
    let (sender, id) = selected_sender();
    let archive = sender.scratch("move.carrova");
    let summary = block_on(send::export_archive(
        &sender.state,
        &id,
        archive.clone(),
        PASS.into(),
        KdfStrength::default(),
    ))
    .unwrap();
    assert!(summary.bytes > 0);
    assert!(engine_events(&sender, &id).contains(&"finished".to_owned()));
    let ended = sender.events.recorded.of_kind("task_ended");
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0]["error"], Value::Null);

    let receiver = Machine::new();
    let opened = block_on(receive::open_archive(&receiver.state, archive, PASS.into())).unwrap();
    let review = block_on(receive::plan(
        &receiver.state,
        &opened.session,
        PlanOptions::default(),
    ))
    .unwrap();
    let review = review.review();
    assert!(review.plan.blockers.is_empty());
    assert_eq!(review.offered[0].key.as_str(), "app/config");
    // The fake sender has no device name.
    assert_eq!(
        review.source,
        Some(SourceDevice {
            os: OsKind::Linux,
            device_name: None,
        })
    );
    block_on(receive::approve(&receiver.state, &opened.session)).unwrap();
    let checklist = block_on(receive::apply(&receiver.state, &opened.session)).unwrap();

    assert_eq!(checklist.moved.len(), 1);
    assert_eq!(
        std::fs::read(receiver.home.join(".config/app/settings.toml")).unwrap(),
        b"theme = 'dark'\n"
    );
    assert!(engine_events(&receiver, &opened.session).contains(&"action_applied".to_owned()));
    let history = block_on(sessions::list(&receiver.state)).unwrap();
    assert_eq!(history[0].id, opened.session);
}

#[test]
fn cancel_stops_a_running_export_and_leaves_no_archive() {
    let (sender, id) = selected_sender();
    let registry = sender.state.sessions.clone();
    let session = id.clone();
    // Cancel from the event stream, i.e. while the export call runs.
    sender.events.on_event(move |event| {
        if event["event"]["event"] == "state_changed" && event["event"]["to"] == "transferring" {
            registry.cancel(&session);
        }
    });
    let archive = sender.scratch("move.carrova");
    let err = block_on(send::export_archive(
        &sender.state,
        &id,
        archive.clone(),
        PASS.into(),
        KdfStrength::default(),
    ))
    .unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Cancelled);
    assert!(!archive.exists());
    let ended = sender.events.recorded.of_kind("task_ended");
    assert_eq!(ended[0]["error"]["code"], "cancelled");
}

#[test]
fn a_new_archive_needs_a_strong_enough_passphrase() {
    let (sender, id) = selected_sender();
    let err = block_on(send::export_archive(
        &sender.state,
        &id,
        sender.scratch("move.carrova"),
        "hunter2".into(),
        KdfStrength::default(),
    ))
    .unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::WrongPassphrase);
    assert!(err.bridge().message.contains("weak"), "{err}");
}

#[test]
fn a_wrong_passphrase_is_reported_as_such() {
    let (sender, id) = selected_sender();
    let archive = sender.scratch("move.carrova");
    block_on(send::export_archive(
        &sender.state,
        &id,
        archive.clone(),
        PASS.into(),
        KdfStrength::default(),
    ))
    .unwrap();
    let receiver = Machine::new();
    let err = block_on(receive::open_archive(
        &receiver.state,
        archive,
        "wrong".into(),
    ))
    .unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::WrongPassphrase);
    assert!(err.bridge().hint.is_some());
}

#[test]
fn resuming_an_archive_import_asks_for_the_passphrase() {
    let (sender, id) = selected_sender();
    let archive = sender.scratch("move.carrova");
    block_on(send::export_archive(
        &sender.state,
        &id,
        archive.clone(),
        PASS.into(),
        KdfStrength::default(),
    ))
    .unwrap();
    let receiver = Machine::new();
    let opened = block_on(receive::open_archive(&receiver.state, archive, PASS.into())).unwrap();
    let err = block_on(sessions::resume(&receiver.state, &opened.session, None)).unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::WrongPassphrase);
    let resumed = block_on(sessions::resume(
        &receiver.state,
        &opened.session,
        Some(PASS.into()),
    ))
    .unwrap();
    assert_eq!(resumed.session, opened.session);
    block_on(receive::plan(
        &receiver.state,
        &opened.session,
        PlanOptions::default(),
    ))
    .unwrap();
}

#[test]
fn approve_refuses_without_a_plan() {
    let (sender, id) = selected_sender();
    let archive = sender.scratch("move.carrova");
    block_on(send::export_archive(
        &sender.state,
        &id,
        archive.clone(),
        PASS.into(),
        KdfStrength::default(),
    ))
    .unwrap();
    let receiver = Machine::new();
    let opened = block_on(receive::open_archive(&receiver.state, archive, PASS.into())).unwrap();
    let err = block_on(receive::approve(&receiver.state, &opened.session)).unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Session);
}

#[test]
fn a_resumed_import_continues_from_its_own_options_and_approval() {
    let (sender, id) = selected_sender();
    let archive = sender.scratch("move.carrova");
    block_on(send::export_archive(
        &sender.state,
        &id,
        archive.clone(),
        PASS.into(),
        KdfStrength::default(),
    ))
    .unwrap();
    let receiver = Machine::new();
    let opened = block_on(receive::open_archive(&receiver.state, archive, PASS.into())).unwrap();
    let stored = || block_on(receive::plan_options(&receiver.state, &opened.session)).unwrap();
    let plan = |options: PlanOptions| {
        block_on(receive::plan(&receiver.state, &opened.session, options))
            .unwrap()
            .review()
            .approval
    };
    assert_eq!(stored(), None, "nothing planned yet");
    let chosen = PlanOptions {
        default_rule: carrova_core::ConflictRule::Skip,
        ..PlanOptions::default()
    };
    assert_eq!(plan(chosen.clone()), PlanApproval::Needed);
    block_on(receive::approve(&receiver.state, &opened.session)).unwrap();
    block_on(sessions::resume(
        &receiver.state,
        &opened.session,
        Some(PASS.into()),
    ))
    .unwrap();

    assert_eq!(
        stored(),
        Some(chosen.clone()),
        "the user's options, not defaults"
    );
    assert_eq!(plan(chosen), PlanApproval::Given);
    assert_eq!(plan(PlanOptions::default()), PlanApproval::Needed);
}
