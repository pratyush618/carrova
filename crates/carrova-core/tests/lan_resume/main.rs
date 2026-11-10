//! LAN sessions surviving interruptions: dropped connections, restarts of
//! either side, a full disk. Crashes are simulated by unwinding a side's
//! thread from its event sink (the engine and its connection are dropped,
//! as when the process dies); session state on disk is what remains.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use carrova_core::{Event, Exporter, Importer, Phase, PlanOptions, SendOutcome, State};
use lan_support::{Side, crash, loopback_broadcast_works, run};

fn options() -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    }
}

fn is_transfer_progress(e: &Event) -> bool {
    matches!(e, Event::Progress { phase, .. } if *phase == Phase::Transfer)
}

#[test]
fn sender_crash_mid_transfer_then_resume_send_completes_without_refetching() {
    if !loopback_broadcast_works() {
        return;
    }
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let id = exp.id().clone();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();

    // The sender dies after serving its first progress step.
    sender.sink.set_hook(|e| {
        if matches!(e, Event::Progress { phase, .. } if *phase == Phase::Send) {
            crash();
        }
    });
    let applying = thread::spawn(move || {
        let r = imp.apply();
        (r, imp)
    });
    assert!(running.thread.join().is_err(), "sender should have crashed");
    sender.sink.clear_hook();
    assert_eq!(
        carrova_core::sessions::show(&sender.engine, &id)
            .unwrap()
            .state,
        State::Transferring
    );
    receiver.sink.wait_for("peer lost", |e| {
        matches!(e, Event::PeerLost { retrying: true })
    });

    // Restarted sender: same session, no code, the receiver reconnects.
    let handle = Exporter::resume_send(sender.engine.clone(), Some(&id)).unwrap();
    assert!(handle.ticket().is_none());
    let resumed = run(handle);
    let (applied, imp) = applying.join().unwrap();
    applied.unwrap();
    assert_eq!(imp.state(), State::Done);
    assert_eq!(
        resumed.thread.join().unwrap().unwrap().outcome,
        SendOutcome::Applied
    );

    // One transfer run: every needed byte fetched exactly once.
    let progress = receiver.progress("transfer");
    assert_eq!(progress.iter().filter(|(d, _)| *d == 0).count(), 1);
    let (done, total) = *progress.last().unwrap();
    assert_eq!(done, total);
    assert_eq!(
        receiver.snap()[&std::path::PathBuf::from(".local/share/app/big.bin")],
        sender.snap()[&std::path::PathBuf::from(".local/share/app/big.bin")]
    );
}

#[test]
fn receiver_crash_mid_transfer_then_resume_lan_fetches_only_what_was_not_checkpointed() {
    if !loopback_broadcast_works() {
        return;
    }
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    let id = imp.id().clone();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();

    // First progress step: linger past the checkpoint interval (the next
    // chunk checkpoints everything so far); second step: crash.
    let steps = Arc::new(AtomicU32::new(0));
    receiver.sink.set_hook(move |e| {
        if let Event::Progress { phase, done, .. } = e
            && *phase == Phase::Transfer
            && *done > 0
        {
            match steps.fetch_add(1, Ordering::SeqCst) {
                0 => thread::sleep(Duration::from_millis(2300)),
                _ => crash(),
            }
        }
    });
    let crashed = thread::spawn(move || imp.apply());
    assert!(crashed.join().is_err(), "receiver should have crashed");
    receiver.sink.clear_hook();
    let (_, needed) = receiver.progress("transfer")[0];
    assert_eq!(
        carrova_core::sessions::show(&receiver.engine, &id)
            .unwrap()
            .state,
        State::Transferring
    );

    // Restarted receiver: same plan, approval kept, only the rest moves.
    let mut imp = Importer::resume_lan(receiver.engine.clone(), Some(&id)).unwrap();
    assert_eq!(imp.state(), State::Planned);
    assert!(imp.record().approved);
    let before = receiver.progress("transfer").len();
    imp.apply().unwrap();
    let second = &receiver.progress("transfer")[before..];
    let (done, total) = *second.last().unwrap();
    assert_eq!(done, total);
    assert!(
        total + 4 * 1024 * 1024 <= needed,
        "resume fetched {total} of {needed} bytes"
    );
    assert_eq!(
        running.thread.join().unwrap().unwrap().outcome,
        SendOutcome::Applied
    );
    assert!(
        sender
            .sink
            .events()
            .iter()
            .any(|e| matches!(e, Event::PeerLost { retrying: true }))
    );
}

#[test]
fn low_disk_space_pauses_the_transfer_until_space_frees_up() {
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();

    receiver.platform.override_free_space(Some(1024));
    let applying = thread::spawn(move || imp.apply());
    receiver
        .sink
        .wait_for("pause", |e| matches!(e, Event::TransferPaused { .. }));
    assert!(
        !receiver
            .sink
            .events()
            .iter()
            .any(is_transfer_progress_with_bytes)
    );
    receiver.platform.override_free_space(None);
    applying.join().unwrap().unwrap();

    let events = receiver.sink.events();
    let paused = events
        .iter()
        .position(|e| matches!(e, Event::TransferPaused { .. }))
        .unwrap();
    let resumed = events
        .iter()
        .position(|e| matches!(e, Event::TransferResumed))
        .unwrap();
    assert!(paused < resumed);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::TransferPaused { .. }))
            .count(),
        1
    );
    running.thread.join().unwrap().unwrap();
}

fn is_transfer_progress_with_bytes(e: &Event) -> bool {
    is_transfer_progress(e) && matches!(e, Event::Progress { done, .. } if *done > 0)
}
