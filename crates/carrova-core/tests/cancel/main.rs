//! Cancellation: a [`carrova_core::Cancel`] taken from a running session
//! object stops it at the next item, file or chunk; the LAN peer is told;
//! nothing reaches the user's home; an apply already running always
//! completes (or rolls back).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;

use std::thread;

use carrova_core::{
    Cancel, CoreError, Event, Importer, KdfParams, Passphrase, Phase, PlanOptions, State,
    WriteOptions, sessions,
};
use lan_support::{Side, run};

fn options() -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    }
}

/// Some bytes of `phase` are done.
fn made_progress(e: &Event, phase: Phase) -> bool {
    matches!(e, Event::Progress { phase: p, done, .. } if *p == phase && *done > 0)
}

/// Cancel `cancel` once `side` reports progress in `phase`.
fn cancel_on_progress(side: &Side, cancel: Cancel, phase: Phase) {
    side.sink.set_hook(move |e| {
        if made_progress(e, phase) {
            cancel.cancel();
        }
    });
}

fn pass() -> Passphrase {
    Passphrase::new("correct horse battery staple".into())
}

fn write_opts() -> WriteOptions {
    WriteOptions {
        kdf: KdfParams::FLOOR,
    }
}

#[test]
fn cancel_is_shared_by_every_clone() {
    let cancel = Cancel::default();
    let other = cancel.clone();
    assert!(!other.is_cancelled());
    cancel.cancel();
    assert!(other.is_cancelled());
    assert!(matches!(other.check(), Err(CoreError::Cancelled)));
}

#[test]
fn receiver_cancelling_mid_transfer_tells_the_sender_and_keeps_staging_resumable() {
    let sender = Side::sender();
    let receiver = Side::new();
    let before = receiver.snap();
    let mut exp = sender.selected_exporter();
    let send_id = exp.id().clone();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    cancel_on_progress(&receiver, imp.canceller(), Phase::Transfer);

    let err = imp.apply().unwrap_err();

    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert_eq!(imp.state(), State::Planned);
    assert!(imp.record().approved, "approval kept for a resume");
    assert!(
        receiver.session_dir(imp.id()).join("staging").exists(),
        "staging kept for a resume"
    );
    assert_eq!(receiver.snap(), before, "nothing written to home");
    let sent = running.thread.join().unwrap().unwrap_err();
    assert!(matches!(sent, CoreError::PeerCancelled), "{sent}");
    assert_eq!(
        sessions::show(&sender.engine, &send_id).unwrap().state,
        State::Transferring,
        "the send stays resumable"
    );
}

#[test]
fn sender_cancelling_mid_transfer_tells_the_receiver() {
    let sender = Side::sender();
    let receiver = Side::new();
    let before = receiver.snap();
    let mut exp = sender.selected_exporter();
    let send_id = exp.id().clone();
    let handle = exp.prepare_send().unwrap();
    cancel_on_progress(&sender, handle.canceller(), Phase::Send);
    let running = run(handle);
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();

    let err = imp.apply().unwrap_err();

    assert!(matches!(err, CoreError::PeerCancelled), "{err}");
    assert_eq!(imp.state(), State::Planned);
    assert_eq!(receiver.snap(), before, "nothing written to home");
    let sent = running.thread.join().unwrap().unwrap_err();
    assert!(matches!(sent, CoreError::Cancelled), "{sent}");
    assert_eq!(
        sessions::show(&sender.engine, &send_id).unwrap().state,
        State::Transferring,
        "the send stays resumable"
    );
}

#[test]
fn sender_cancelling_while_waiting_for_a_receiver_returns_to_the_selection() {
    let sender = Side::sender();
    let mut exp = sender.selected_exporter();
    let send_id = exp.id().clone();
    let handle = exp.prepare_send().unwrap();
    let cancel = handle.canceller();
    let running = run(handle);
    thread::sleep(std::time::Duration::from_millis(200));

    cancel.cancel();

    let err = running.thread.join().unwrap().unwrap_err();
    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert_eq!(
        sessions::show(&sender.engine, &send_id).unwrap().state,
        State::Selected,
        "never paired: nothing to resume, the selection can be sent again"
    );
}

#[test]
fn cancelled_export_leaves_no_partial_archive() {
    let sender = Side::sender();
    let mut exp = sender.selected_exporter();
    let archive = sender.tmp.path().join("out.carrova");
    cancel_on_progress(&sender, exp.canceller(), Phase::Archive);

    let err = exp.export(&archive, &pass(), write_opts()).unwrap_err();

    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert!(!archive.exists(), "partial archive removed");
    assert_eq!(exp.state(), State::Failed);
}

/// An approved archive import of the sender's data on a fresh receiver.
fn approved_archive_import(sender: &Side, receiver: &Side) -> Importer {
    let archive = sender.tmp.path().join("out.carrova");
    sender
        .selected_exporter()
        .export(&archive, &pass(), write_opts())
        .unwrap();
    let mut imp = Importer::open(receiver.engine.clone(), &archive, &pass()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    imp
}

#[test]
fn cancel_while_staging_writes_nothing_to_home_and_the_import_can_resume() {
    let sender = Side::sender();
    let receiver = Side::new();
    let before = receiver.snap();
    let mut imp = approved_archive_import(&sender, &receiver);
    cancel_on_progress(&receiver, imp.canceller(), Phase::Stage);

    let err = imp.apply().unwrap_err();

    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert_eq!(receiver.snap(), before, "nothing written to home");
    assert_eq!(imp.state(), State::Planned);
    assert!(imp.record().approved);
    receiver.sink.clear_hook();
    let id = imp.id().clone();
    drop(imp);
    let mut resumed = Importer::resume_archive(receiver.engine.clone(), &id, &pass()).unwrap();
    resumed.apply().unwrap();
    assert_eq!(resumed.state(), State::Done);
}

#[test]
fn cancel_signalled_during_apply_is_ignored_until_apply_completes() {
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    let cancel = imp.canceller();
    receiver.sink.set_hook(move |e| {
        if matches!(
            e,
            Event::StateChanged {
                to: State::Applying,
                ..
            }
        ) {
            cancel.cancel();
        }
    });

    let checklist = imp.apply().unwrap();

    assert_eq!(imp.state(), State::Done);
    assert!(imp.canceller().is_cancelled());
    assert_eq!(checklist.moved.len(), 3);
    running.thread.join().unwrap().unwrap();
}
