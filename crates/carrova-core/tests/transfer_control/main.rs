//! The user's live controls over a running LAN transfer: pause/resume and
//! the "continue anyway" override of a low-disk-space pause.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;

use std::thread;
use std::time::Duration;

use carrova_core::{Event, Importer, PauseReason, Phase, PlanOptions, State};
use lan_support::{Side, loopback_broadcast_works, run};

fn options() -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ignore_free_space: true,
        ..PlanOptions::default()
    }
}

fn paused_for_space(e: &Event) -> bool {
    matches!(
        e,
        Event::TransferPaused {
            reason: PauseReason::LowDiskSpace { .. }
        }
    )
}

fn fetched_bytes(e: &Event) -> bool {
    matches!(e, Event::Progress { phase: Phase::Transfer, done, .. } if *done > 0)
}

#[test]
fn ignoring_free_space_continues_a_transfer_paused_for_low_disk_space() {
    if !loopback_broadcast_works() {
        return;
    }
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    let control = imp.transfer_control();

    // Below the transfer's safety margin, but the staged files fit.
    receiver.platform.override_free_space(Some(1024));
    let applying = thread::spawn(move || imp.apply().map(|_| imp));
    receiver.sink.wait_for("pause", paused_for_space);
    control.ignore_free_space();
    let imp = applying.join().unwrap().unwrap();

    assert_eq!(imp.state(), State::Done);
    let events = receiver.sink.events();
    let paused = events.iter().position(paused_for_space).unwrap();
    let resumed = events
        .iter()
        .position(|e| matches!(e, Event::TransferResumed))
        .unwrap();
    assert!(paused < resumed);
    let Event::TransferPaused {
        reason: PauseReason::LowDiskSpace { disk, .. },
    } = &events[paused]
    else {
        unreachable!()
    };
    assert!(disk.is_some(), "the pause names the disk");
    running.thread.join().unwrap().unwrap();
}

#[test]
fn a_user_pause_holds_the_transfer_until_resumed() {
    if !loopback_broadcast_works() {
        return;
    }
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    let control = imp.transfer_control();

    control.pause();
    let applying = thread::spawn(move || imp.apply().map(|_| imp));
    receiver.sink.wait_for("pause", |e| {
        matches!(
            e,
            Event::TransferPaused {
                reason: PauseReason::ByUser
            }
        )
    });
    thread::sleep(Duration::from_millis(300));
    assert!(
        !receiver.sink.events().iter().any(fetched_bytes),
        "nothing is fetched while paused"
    );
    control.resume();
    let imp = applying.join().unwrap().unwrap();

    assert_eq!(imp.state(), State::Done);
    assert!(
        receiver
            .sink
            .events()
            .iter()
            .any(|e| matches!(e, Event::TransferResumed))
    );
    running.thread.join().unwrap().unwrap();
}
