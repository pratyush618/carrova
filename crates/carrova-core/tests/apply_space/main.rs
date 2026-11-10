//! Free space re-checked once an approved plan is applied: a disk that
//! filled up after approval stops an archive import before anything is
//! written, and pauses a LAN import (naming the short disk) until space
//! frees up or the user overrides the check.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/archive.rs"]
mod archive_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;

use std::thread;

use carrova_core::{CoreError, Event, Importer, PauseReason, Phase, PlanOptions, State};
use lan_support::{Side, loopback_broadcast_works, run};

#[test]
fn an_archive_import_short_of_space_after_approval_writes_nothing_and_applies_once_space_returns() {
    let r = archive_support::Side::new();
    common::mkdir(&*r.platform, &r.home, ".config/app", 0o755);
    r.put(".config/app/settings.json", b"receiver settings");
    let mut imp = Importer::open(
        r.engine.clone(),
        &archive_support::archive().path,
        &archive_support::pass(),
    )
    .unwrap();
    imp.plan(&PlanOptions::default()).unwrap();
    imp.approve().unwrap();

    r.platform.override_free_space(Some(1024));
    let err = imp.apply().unwrap_err();

    assert!(matches!(err, CoreError::NotEnoughSpace { .. }), "{err}");
    assert!(
        err.to_string().contains("user_config"),
        "names the disk: {err}"
    );
    assert_eq!(imp.state(), State::Planned);
    assert!(imp.record().approved);
    let settings = r.home.join(".config/app/settings.json");
    assert_eq!(std::fs::read(&settings).unwrap(), b"receiver settings");
    assert!(!r.home.join(".config/app/new.txt").exists());

    r.platform.override_free_space(None);
    imp.apply().unwrap();
    assert_eq!(imp.state(), State::Done);
    assert!(r.home.join(".config/app/new.txt").exists());
}

fn options(ignore_free_space: bool) -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ignore_free_space,
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

fn transfer_done(e: &Event) -> bool {
    matches!(e, Event::Progress { phase: Phase::Transfer, done, total } if *done > 0 && done == total)
}

/// A LAN import into `receiver` (home on its own disk) whose disks report
/// 1 KiB free from the end of the transfer until it pauses for space.
fn lan_import_short_before_placement(receiver: &Side, ignore_free_space: bool) -> Importer {
    let sender = Side::sender();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options(ignore_free_space)).unwrap();
    imp.approve().unwrap();

    let platform = receiver.platform.clone();
    let mut armed = true;
    receiver.sink.set_hook(move |e| {
        if armed && transfer_done(e) {
            armed = false;
            platform.override_free_space(Some(1024));
        } else if paused_for_space(e) {
            platform.override_free_space(None);
        }
    });
    let applying = thread::spawn(move || imp.apply().map(|_| imp));
    let imp = applying.join().unwrap().unwrap();
    running.thread.join().unwrap().unwrap();
    imp
}

#[test]
fn a_lan_import_whose_target_disk_runs_short_before_placement_pauses_naming_it() {
    if !loopback_broadcast_works() {
        return;
    }
    let receiver = Side::home_on_own_disk();
    let imp = lan_import_short_before_placement(&receiver, false);

    assert_eq!(imp.state(), State::Done);
    let events = receiver.sink.events();
    let paused = events.iter().position(paused_for_space).unwrap();
    let resumed = events
        .iter()
        .position(|e| matches!(e, Event::TransferResumed))
        .unwrap();
    assert!(paused < resumed);
    let Event::TransferPaused {
        reason: PauseReason::LowDiskSpace { fs, disk, .. },
    } = &events[paused]
    else {
        unreachable!()
    };
    assert_eq!(disk.as_deref(), Some(&*receiver.home.display().to_string()));
    assert!(
        !fs.contains("state dir"),
        "the target disk, not staging: {fs}"
    );
}

#[test]
fn a_lan_import_approved_with_the_space_override_does_not_pause_before_placement() {
    if !loopback_broadcast_works() {
        return;
    }
    let receiver = Side::home_on_own_disk();
    let imp = lan_import_short_before_placement(&receiver, true);

    assert_eq!(imp.state(), State::Done);
    assert!(!receiver.sink.events().iter().any(paused_for_space));
}
