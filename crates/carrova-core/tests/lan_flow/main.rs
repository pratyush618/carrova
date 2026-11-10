//! LAN sessions end to end: two engines in one process over loopback.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;

use std::collections::BTreeSet;

use std::time::Duration;

use carrova_core::lan::{ConflictTarget, MAX_PAIRING_FAILURES, PlanSummary};
use carrova_core::store::{LOG_FILE, RESUME_KEY_FILE};
use carrova_core::{
    CoreError, Event, Importer, ItemKey, KdfParams, Passphrase, PlanOptions, SendOutcome, Source,
    State, Ticket, WriteOptions, sessions,
};
use carrova_crypto::pairing::PairingCode;
use carrova_platform::{OsKind, SemanticRoot};
use lan_support::{Side, run, run_answering};

fn options() -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    }
}

/// A different code on the same nameplate.
fn other_words(code: &PairingCode) -> PairingCode {
    loop {
        let fresh = PairingCode::generate().unwrap().to_string();
        let words = fresh.split_once('-').unwrap().1;
        let other: PairingCode = format!("{}-{words}", code.nameplate()).parse().unwrap();
        if &other != code {
            return other;
        }
    }
}

#[test]
fn lan_migration_lands_the_same_files_and_modes_as_an_archive_migration() {
    let sender = Side::sender();

    // Archive path.
    let archive = sender.tmp.path().join("out.carrova");
    let pass = Passphrase::new("correct horse battery staple".into());
    sender
        .selected_exporter()
        .export(
            &archive,
            &pass,
            WriteOptions {
                kdf: KdfParams::FLOOR,
            },
        )
        .unwrap();
    let by_archive = Side::new();
    let mut imp = Importer::open(by_archive.engine.clone(), &archive, &pass).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    imp.apply().unwrap();

    // LAN path.
    let by_lan = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let send_id = exp.id().clone();
    let mut imp =
        Importer::connect(by_lan.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    assert_eq!(imp.state(), State::Selected);
    assert_eq!(imp.manifest().unwrap().items.len(), 3);
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    let checklist = imp.apply().unwrap();
    assert_eq!(imp.state(), State::Done);
    let report = running.thread.join().unwrap().unwrap();

    assert_eq!(by_lan.snap(), by_archive.snap());
    assert_eq!(report.outcome, SendOutcome::Applied);
    assert_eq!(report.checklist, checklist);
    assert_eq!(
        checklist.moved,
        ["app/config", "app/data", "app/keys"].map(ItemKey::from)
    );
    assert!(report.bytes >= lan_support::BIG as u64);

    // Sender saw the inventory and plan summary, and finished.
    let events = sender.sink.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::InventoryReceived(_)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::PlanSummaryReceived(_)))
    );
    assert_eq!(
        sessions::show(&sender.engine, &send_id).unwrap().state,
        State::Done
    );

    // Receiver progress covers exactly the bytes it needed.
    let (done, total) = *by_lan.progress("transfer").last().unwrap();
    assert_eq!(done, total);
    assert!(total >= lan_support::BIG as u64);

    // Resume secrets are gone on both sides once Done.
    assert!(!sender.session_dir(&send_id).join(RESUME_KEY_FILE).exists());
    assert!(!by_lan.session_dir(imp.id()).join(RESUME_KEY_FILE).exists());
}

#[test]
fn wrong_code_burns_it_and_the_fresh_code_pairs_without_codes_reaching_events_or_logs() {
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let first = running.first.clone().unwrap();

    let wrong = Ticket {
        code: other_words(&first.code),
        addr: first.addr,
    };
    let err = Importer::connect(receiver.engine.clone(), &wrong).unwrap_err();
    assert!(matches!(err, CoreError::Transport(_)), "{err}");
    let fresh = running
        .tickets
        .recv_timeout(std::time::Duration::from_secs(30))
        .unwrap();
    assert_ne!(fresh.code, first.code);
    sender
        .sink
        .wait_for("pairing failure", |e| matches!(e, Event::PairingFailed));
    assert_eq!(
        running.asked(),
        [(1, MAX_PAIRING_FAILURES - 1)],
        "the user is asked before a new code is offered"
    );

    // The burned code no longer works (another nameplate now: turned
    // away without burning the fresh code); the fresh one does.
    assert!(Importer::connect(receiver.engine.clone(), &first).is_err());
    let mut imp = Importer::connect(receiver.engine.clone(), &fresh).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();
    imp.apply().unwrap();
    running.thread.join().unwrap().unwrap();

    let mut texts = vec![
        serde_json::to_string(&sender.sink.events()).unwrap(),
        serde_json::to_string(&receiver.sink.events()).unwrap(),
        std::fs::read_to_string(sender.session_dir(exp.id()).join(LOG_FILE)).unwrap(),
        std::fs::read_to_string(receiver.session_dir(imp.id()).join(LOG_FILE)).unwrap(),
    ];
    texts.push(std::fs::read_to_string(sender.session_dir(exp.id()).join("session.json")).unwrap());
    for code in [&first.code, &wrong.code, &fresh.code] {
        let code = code.to_string();
        let words = code.split_once('-').unwrap().1;
        for t in &texts {
            assert!(!t.contains(words), "a pairing code leaked");
        }
    }
}

/// A wrong code entered on a receiver against `ticket`'s nameplate.
fn guess_wrong(receiver: &Side, ticket: &Ticket) {
    let wrong = Ticket {
        code: other_words(&ticket.code),
        addr: ticket.addr,
    };
    let err = Importer::connect(receiver.engine.clone(), &wrong).unwrap_err();
    assert!(matches!(err, CoreError::Transport(_)), "{err}");
}

#[test]
fn third_failed_pairing_ends_the_send_with_attempts_exhausted() {
    let sender = Side::sender();
    let guesser = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut ticket = running.first.clone().unwrap();
    for _ in 1..MAX_PAIRING_FAILURES {
        guess_wrong(&guesser, &ticket);
        ticket = running
            .tickets
            .recv_timeout(Duration::from_secs(30))
            .unwrap();
    }
    guess_wrong(&guesser, &ticket);

    let (result, asked, unseen) = running.finish();
    let err = result.unwrap_err();
    assert!(
        matches!(err, CoreError::PairingAttemptsExhausted { failures } if failures == MAX_PAIRING_FAILURES),
        "{err}"
    );
    assert_eq!(asked, [(1, 2), (2, 1)]);
    assert!(unseen.is_empty(), "no code after the last allowed failure");
    let record = sessions::show(&sender.engine, exp.id()).unwrap();
    assert_eq!(record.state, State::Failed);
    let failures = sender
        .sink
        .events()
        .iter()
        .filter(|e| matches!(e, Event::PairingFailed))
        .count();
    assert_eq!(failures, MAX_PAIRING_FAILURES as usize);
}

#[test]
fn declining_a_new_code_ends_the_send_without_offering_one_and_keeps_the_selection() {
    let sender = Side::sender();
    let guesser = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run_answering(exp.prepare_send().unwrap(), false);
    let first = running.first.clone().unwrap();
    guess_wrong(&guesser, &first);

    let (result, asked, unseen) = running.finish();
    let err = result.unwrap_err();
    assert!(
        matches!(err, CoreError::PairingStopped { failures: 1 }),
        "{err}"
    );
    assert_eq!(asked, [(1, MAX_PAIRING_FAILURES - 1)]);
    assert!(unseen.is_empty(), "no new code offered");
    assert_eq!(
        sessions::show(&sender.engine, exp.id()).unwrap().state,
        State::Selected
    );
    // The listener is gone: even the right (burned) code reaches nobody.
    assert!(Importer::connect(guesser.engine.clone(), &first).is_err());
}

#[test]
fn receiver_knows_the_senders_os_and_device_name_before_planning() {
    let sender = Side::seeded(Side::named(Some("ezio-thinkpad")));
    let receiver = Side::new();

    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();

    let source = &imp.manifest().unwrap().source;
    assert_eq!(source.os, OsKind::Linux);
    assert_eq!(source.device_name.as_deref(), Some("ezio-thinkpad"));
    let record = sessions::show(&receiver.engine, imp.id()).unwrap();
    assert!(matches!(
        record.source,
        Some(Source::Lan { peer_name: Some(ref n), .. }) if n == "ezio-thinkpad"
    ));
    imp.plan(&options()).unwrap();
    imp.end_dry_run().unwrap();
    running.thread.join().unwrap().unwrap();
}

#[test]
fn dry_run_ends_the_sender_cleanly_and_writes_nothing_on_the_receiver() {
    let sender = Side::sender();
    let receiver = Side::new();
    receiver.put(".config/app/settings.json", b"{\"mine\":1}", 0o644);
    let before = receiver.snap();

    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    let dir = receiver.session_dir(imp.id());
    assert!(dir.join(RESUME_KEY_FILE).exists());
    imp.plan(&options()).unwrap();
    imp.end_dry_run().unwrap();

    let report = running.thread.join().unwrap().unwrap();
    assert_eq!(report.outcome, SendOutcome::DryRun);
    assert_eq!(report.bytes, 0);
    assert!(report.checklist.moved.is_empty());
    assert_eq!(report.checklist.skipped.len(), 3);
    assert_eq!(receiver.snap(), before);
    assert!(!dir.join(RESUME_KEY_FILE).exists());
    assert!(Importer::resume_lan(receiver.engine.clone(), Some(imp.id())).is_err());
    assert_eq!(
        sessions::show(&sender.engine, exp.id()).unwrap().state,
        State::Done
    );
}

#[test]
fn plan_summary_lists_counts_and_only_conflicting_receiver_paths() {
    let sender = Side::sender();
    let receiver = Side::new();
    receiver.put(".config/app/settings.json", b"{\"mine\":1}", 0o644);
    receiver.put(".config/app/private-unrelated.txt", b"x", 0o644);
    receiver.put("unrelated.txt", b"y", 0o644);

    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    let plan = imp.plan(&options()).unwrap().clone();
    imp.approve().unwrap();
    sender.sink.wait_for("plan summary", |e| {
        matches!(e, Event::PlanSummaryReceived(_))
    });
    let summary: PlanSummary = sender
        .sink
        .events()
        .into_iter()
        .find_map(|e| match e {
            Event::PlanSummaryReceived(s) => Some(s),
            _ => None,
        })
        .unwrap();
    assert_eq!(summary, PlanSummary::from_plan(&plan));
    let conflicts: Vec<&ConflictTarget> = summary.items.iter().flat_map(|i| &i.conflicts).collect();
    let rels: BTreeSet<&str> = conflicts
        .iter()
        .inspect(|c| assert_eq!(c.root, SemanticRoot::UserConfig))
        .map(|c| c.rel.as_str())
        .collect();
    // The existing dir (mode tightened) and the differing file; nothing
    // else of the receiver's.
    assert_eq!(rels, BTreeSet::from(["app", "app/settings.json"]));
    let config = summary
        .items
        .iter()
        .find(|i| i.item == "app/config")
        .unwrap();
    assert_eq!(config.counts.replace, 2);
    assert!(config.counts.create >= 1);
    let wire = serde_json::to_string(&summary).unwrap();
    assert!(!wire.contains("unrelated"));

    imp.apply().unwrap();
    running.thread.join().unwrap().unwrap();
}

#[test]
fn file_changed_on_the_sender_fails_the_transfer_back_to_planning_then_replan_succeeds() {
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let mut imp =
        Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&options()).unwrap();
    imp.approve().unwrap();

    // Same size, other bytes: the sender can no longer supply its chunks.
    sender.put(
        ".local/share/app/big.bin",
        &lan_support::pseudo_random(lan_support::BIG, 99),
        0o600,
    );
    let err = imp.apply().unwrap_err();
    let CoreError::TransferIncomplete { items } = &err else {
        panic!("{err}");
    };
    assert_eq!(items, &[ItemKey::from("app/data")]);
    assert_eq!(imp.state(), State::Planned);
    assert!(!imp.record().approved);
    assert!(!receiver.home.join(".local/share/app/big.bin").exists());

    // Re-plan without the item, approve, apply: done.
    imp.plan(&PlanOptions {
        items: Some(vec!["app/config".into(), "app/keys".into()]),
        ..options()
    })
    .unwrap();
    imp.approve().unwrap();
    let checklist = imp.apply().unwrap();
    let moved: BTreeSet<&str> = checklist.moved.iter().map(ItemKey::as_str).collect();
    assert_eq!(moved, BTreeSet::from(["app/config", "app/keys"]));
    assert_eq!(
        running.thread.join().unwrap().unwrap().outcome,
        SendOutcome::Applied
    );
}

#[test]
fn dismiss_wipes_the_resume_secret() {
    let sender = Side::sender();
    let receiver = Side::new();
    let mut exp = sender.selected_exporter();
    let running = run(exp.prepare_send().unwrap());
    let imp = Importer::connect(receiver.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    let id = imp.id().clone();
    let key = receiver.session_dir(&id).join(RESUME_KEY_FILE);
    assert!(key.exists());
    drop(imp);
    sessions::dismiss(&receiver.engine, &id).unwrap();
    assert!(!key.exists());
    drop(running);
}

#[test]
fn resume_without_id_picks_the_newest_lan_session_of_the_role_never_an_archive_one() {
    use carrova_core::store::{Source, Store};
    use carrova_core::{Exporter, Role};
    let side = Side::new();
    let platform = &*side.platform;
    let store = Store::open(platform).unwrap();
    let lan = || Source::Lan {
        peer_cert_hash: None,
        manifest_fingerprint: carrova_manifest::Blake3Hash::of(b"m"),
        peer_name: None,
    };
    let lan_import = store
        .create(platform, Role::Import, State::Planned, Some(lan()))
        .unwrap();
    store
        .create(platform, Role::Import, State::Done, Some(lan()))
        .unwrap();
    store
        .create(
            platform,
            Role::Import,
            State::Planned,
            Some(Source::Archive {
                path: side.tmp.path().join("a.carrova"),
                fingerprint: None,
            }),
        )
        .unwrap();

    // Picked (then refused before any network use: it never paired).
    let err = Importer::resume_lan(side.engine.clone(), None).unwrap_err();
    assert!(err.to_string().contains(lan_import.id.0.as_str()), "{err}");

    let err = Exporter::resume_send(side.engine.clone(), None).unwrap_err();
    assert!(
        matches!(err, CoreError::NothingToResume { role: Role::Export }),
        "{err}"
    );
    assert_eq!(err.to_string(), "no interrupted LAN send to resume");
}
