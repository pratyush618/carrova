//! Apply + rollback behaviour with a fake platform and tempdir homes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/apply.rs"]
mod apply_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use apply_support::{Env, Node, action, file, plan, seed_home};
use carrova_core::CoreError;
use carrova_core::plan::{Action, ObjectKind};
use carrova_core::session::{Event, NullSink, Phase, RollbackOutcome, VecSink};
use carrova_core::test_support::apply::{Failpoint, apply};
use carrova_core::test_support::journal::{self, Record};
use carrova_core::test_support::rollback::{dismiss, rollback};
use carrova_platform::SemanticRoot::{Home, UserConfig};

const NO_FAIL: Failpoint = Failpoint::OFF;

#[test]
fn create_files_and_dirs_with_exact_modes() {
    let mut env = Env::new();
    let dir = action(
        &env,
        "ssh",
        0,
        ObjectKind::Dir,
        Home,
        ".ssh",
        0,
        Some(0o700),
        Action::Create,
    );
    let key = file(
        &mut env,
        "ssh",
        1,
        Home,
        ".ssh/id_ed25519",
        b"KEY",
        0o600,
        Action::Create,
    );
    let cfg = file(
        &mut env,
        "app",
        0,
        UserConfig,
        "app/sub/conf.toml",
        b"a = 1",
        0o644,
        Action::Create,
    );
    let wide = file(
        &mut env,
        "app",
        1,
        UserConfig,
        "app/wide",
        b"w",
        0o777,
        Action::Create,
    );
    // Deliberately out of depth order: apply creates `.ssh` first anyway.
    let p = plan(vec![key, cfg, wide, dir], vec![]);
    let sink = VecSink::default();
    let report = apply(
        &env.platform,
        &p,
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &sink,
        NO_FAIL,
    )
    .unwrap();

    let h = &env.home;
    assert_eq!(std::fs::read(h.join(".ssh/id_ed25519")).unwrap(), b"KEY");
    assert_eq!(env.mode(&h.join(".ssh")), 0o700);
    assert_eq!(env.mode(&h.join(".ssh/id_ed25519")), 0o600);
    assert_eq!(env.mode(&h.join(".config/app/sub/conf.toml")), 0o644);
    // Group/other write is never granted (umask-independent otherwise).
    assert_eq!(env.mode(&h.join(".config/app/wide")), 0o755);
    // Intermediate dirs not in the plan are private.
    assert_eq!(env.mode(&h.join(".config/app/sub")), 0o700);
    assert_eq!(report.written.len(), 4);
    assert!(report.backed_up.is_empty());

    // No temp files left behind.
    for (path, _) in env.snap() {
        assert!(!path.to_string_lossy().contains("carrova-tmp"), "{path:?}");
    }
    let recs = journal::read(&env.platform, &env.session_dir.join("journal.cbor")).unwrap();
    assert_eq!(recs.last(), Some(&Record::Committed));
    let events = sink.events();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::ActionApplied { .. }))
            .count(),
        4
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Progress {
            phase: Phase::Apply,
            done: 9,
            total: 9
        }
    )));
}

#[test]
fn replace_backs_up_and_rollback_restores_exactly() {
    let mut env = Env::new();
    seed_home(&env);
    let before = env.snap();
    let a = file(
        &mut env,
        "app",
        0,
        UserConfig,
        "app/settings.json",
        b"{\"new\":1}",
        0o600,
        Action::Replace,
    );
    let b = file(
        &mut env,
        "ssh",
        0,
        Home,
        ".ssh/id_ed25519",
        b"NEW-KEY",
        0o600,
        Action::Replace,
    );
    let c = file(
        &mut env,
        "app",
        1,
        UserConfig,
        "app/extra/new.txt",
        b"n",
        0o644,
        Action::Create,
    );
    let p = plan(vec![a, b, c], vec![]);
    let report = apply(
        &env.platform,
        &p,
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    assert_eq!(report.backed_up.len(), 2);
    let h = &env.home;
    assert_eq!(
        std::fs::read(h.join(".config/app/settings.json")).unwrap(),
        b"{\"new\":1}"
    );
    assert_eq!(env.mode(&h.join(".config/app/settings.json")), 0o600);
    assert_ne!(env.snap(), before);

    let sink = VecSink::default();
    let rb = rollback(&env.platform, &env.session_dir, &sink).unwrap();
    assert!(rb.failed.is_empty(), "{:?}", rb.failed);
    assert_eq!(rb.restored.len(), 2);
    assert_eq!(env.snap(), before);
    // One event per path, with the same outcome as the report.
    let steps = |want: RollbackOutcome| -> Vec<String> {
        sink.events()
            .into_iter()
            .filter_map(|e| match e {
                Event::RollbackStep { target, outcome } if outcome == want => Some(target),
                _ => None,
            })
            .collect()
    };
    assert_eq!(steps(RollbackOutcome::Restored), rb.restored);
    assert_eq!(steps(RollbackOutcome::Removed), rb.removed);
    assert!(!rb.removed.is_empty(), "created files are removed");
    assert!(steps(RollbackOutcome::Failed).is_empty());

    // Second run: nothing left to do, tree unchanged.
    let again = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert_eq!(again, Default::default());
    assert_eq!(env.snap(), before);
    let recs = journal::read(&env.platform, &env.session_dir.join("journal.cbor")).unwrap();
    assert_eq!(recs.iter().filter(|r| **r == Record::RolledBack).count(), 1);
}

#[test]
fn keep_both_skip_and_identical() {
    let mut env = Env::new();
    seed_home(&env);
    let before = env.snap();
    let kb = file(
        &mut env,
        "notes",
        0,
        Home,
        "notes.txt",
        b"incoming",
        0o644,
        Action::KeepBoth {
            rel: "notes.txt.carrova-20261108".into(),
        },
    );
    let skip = file(
        &mut env,
        "ssh",
        0,
        Home,
        ".ssh/id_ed25519",
        b"X",
        0o600,
        Action::Skip {
            reason: "user".into(),
        },
    );
    let same = file(
        &mut env,
        "app",
        0,
        UserConfig,
        "app/settings.json",
        b"{\"old\":true}",
        0o640,
        Action::Identical,
    );
    let p = plan(vec![kb, skip, same], vec![]);
    let sink = VecSink::default();
    let report = apply(
        &env.platform,
        &p,
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &sink,
        NO_FAIL,
    )
    .unwrap();
    assert_eq!(
        report.written,
        vec!["home:notes.txt.carrova-20261108".to_string()]
    );
    assert!(report.backed_up.is_empty());

    let mut after = env.snap();
    let new = after
        .remove(std::path::Path::new("notes.txt.carrova-20261108"))
        .unwrap();
    assert_eq!(
        new,
        Node::File {
            mode: 0o644,
            bytes: b"incoming".to_vec()
        }
    );
    assert_eq!(after, before);
    let actions: Vec<String> = sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::ActionApplied { action, .. } => Some(action),
            _ => None,
        })
        .collect();
    assert!(actions.contains(&"skip".to_string()) && actions.contains(&"identical".to_string()));

    rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert_eq!(env.snap(), before);
}

#[test]
fn keep_both_never_replaces_an_existing_file() {
    let mut env = Env::new();
    seed_home(&env);
    env.put(&env.home.join("notes.txt.kb"), b"already here", 0o600);
    let before = env.snap();
    let kb = file(
        &mut env,
        "n",
        0,
        Home,
        "notes.txt",
        b"in",
        0o644,
        Action::KeepBoth {
            rel: "notes.txt.kb".into(),
        },
    );
    let err = apply(
        &env.platform,
        &plan(vec![kb], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap_err();
    assert!(matches!(err, CoreError::RolledBack { .. }), "{err}");
    assert_eq!(env.snap(), before);
}

#[test]
fn symlink_entries_and_dir_mode_change_roll_back() {
    let env = Env::new();
    seed_home(&env);
    let before = env.snap();
    let mut link = action(
        &env,
        "app",
        0,
        ObjectKind::Symlink,
        UserConfig,
        "app/current",
        0,
        None,
        Action::Create,
    );
    link.link_target = Some("settings.json".into());
    let dir = action(
        &env,
        "app",
        1,
        ObjectKind::Dir,
        UserConfig,
        "app",
        0,
        Some(0o700),
        Action::Replace,
    );
    let p = plan(vec![link, dir], vec![]);
    apply(
        &env.platform,
        &p,
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    let cfg = env.home.join(".config/app");
    assert_eq!(
        std::fs::read_link(cfg.join("current")).unwrap(),
        std::path::Path::new("settings.json")
    );
    assert_eq!(env.mode(&cfg), 0o700);

    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert_eq!(rb.removed.len(), 1);
    assert_eq!(env.snap(), before);

    // A symlink action without a target fails and rolls back.
    let env2 = Env::new();
    let link = action(
        &env2,
        "app",
        0,
        ObjectKind::Symlink,
        UserConfig,
        "x",
        0,
        None,
        Action::Create,
    );
    let err = apply(
        &env2.platform,
        &plan(vec![link], vec![]),
        &env2.manifest(),
        &env2.staged,
        &env2.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap_err();
    assert!(matches!(err, CoreError::RolledBack { .. }), "{err}");
    assert!(env2.snap().is_empty());
}

#[test]
fn symlink_in_ancestor_aborts_and_rolls_back() {
    let mut env = Env::new();
    seed_home(&env);
    let outside = env.tmp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    carrova_platform::fake::symlink(&outside, env.home.join("evil")).unwrap();
    let before = env.snap();
    let ok = file(
        &mut env,
        "a",
        0,
        Home,
        "fresh/ok.txt",
        b"ok",
        0o644,
        Action::Create,
    );
    let rep = file(
        &mut env,
        "a",
        1,
        Home,
        "notes.txt",
        b"new notes",
        0o644,
        Action::Replace,
    );
    let bad = file(
        &mut env,
        "a",
        2,
        Home,
        "evil/sub/payload",
        b"x",
        0o644,
        Action::Create,
    );
    let p = plan(vec![ok, rep, bad], vec![]);
    let err = apply(
        &env.platform,
        &p,
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap_err();
    match err {
        CoreError::RolledBack { cause } => assert!(cause.contains("unsafe target"), "{cause}"),
        e => panic!("unexpected {e}"),
    }
    assert_eq!(env.snap(), before);
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
}

#[test]
fn target_appearing_after_plan_is_not_clobbered() {
    let mut env = Env::new();
    seed_home(&env);
    let before = env.snap();
    // Planned as Create, but the file exists (changed since planning).
    let a = file(
        &mut env,
        "a",
        0,
        Home,
        "notes.txt",
        b"new",
        0o644,
        Action::Create,
    );
    let err = apply(
        &env.platform,
        &plan(vec![a], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap_err();
    assert!(matches!(err, CoreError::RolledBack { .. }), "{err}");
    assert_eq!(env.snap(), before);
}

#[test]
fn plan_abs_mismatch_is_refused() {
    let mut env = Env::new();
    let mut a = file(&mut env, "a", 0, Home, "x.txt", b"x", 0o644, Action::Create);
    a.abs = env.tmp.path().join("elsewhere/x.txt");
    let err = apply(
        &env.platform,
        &plan(vec![a], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap_err();
    assert!(
        matches!(err, CoreError::RolledBack { ref cause } if cause.contains("unsafe")),
        "{err}"
    );
    assert!(env.snap().is_empty());
}

#[test]
fn dotdot_rel_is_refused() {
    let mut env = Env::new();
    let a = file(
        &mut env,
        "a",
        0,
        Home,
        "../escape",
        b"x",
        0o644,
        Action::Create,
    );
    let err = apply(
        &env.platform,
        &plan(vec![a], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap_err();
    assert!(matches!(err, CoreError::RolledBack { .. }), "{err}");
    assert!(!env.tmp.path().join("escape").exists());
}

#[test]
fn blockers_and_existing_journal_refuse_before_writing() {
    let mut env = Env::new();
    let a = file(&mut env, "a", 0, Home, "x.txt", b"x", 0o644, Action::Create);
    let mut p = plan(vec![a], vec![]);
    p.blockers.push(carrova_core::plan::Blocker::RunningApp {
        app: "app".into(),
        name: "app".into(),
    });
    let err = apply(
        &env.platform,
        &p,
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap_err();
    assert!(matches!(err, CoreError::PlanBlocked(_)));
    p.blockers.clear();
    std::fs::write(env.session_dir.join("journal.cbor"), b"").unwrap();
    let err = apply(
        &env.platform,
        &p,
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap_err();
    assert!(!matches!(err, CoreError::RolledBack { .. }), "{err}");
    assert!(env.snap().is_empty());
}

#[test]
fn missing_staged_file_or_size_mismatch_rolls_back() {
    let mut env = Env::new();
    seed_home(&env);
    let before = env.snap();
    let ok = file(&mut env, "a", 0, Home, "new1", b"1", 0o644, Action::Create);
    let mut short = file(
        &mut env,
        "a",
        1,
        Home,
        "notes.txt",
        b"22",
        0o644,
        Action::Replace,
    );
    short.size = 5;
    let err = apply(
        &env.platform,
        &plan(vec![ok, short], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap_err();
    assert!(
        matches!(err, CoreError::RolledBack { ref cause } if cause.contains("verification")),
        "{err}"
    );
    assert_eq!(env.snap(), before);
}

#[test]
fn rollback_tolerates_torn_journal_tail() {
    let mut env = Env::new();
    seed_home(&env);
    let before = env.snap();
    let a = file(
        &mut env,
        "a",
        0,
        Home,
        "notes.txt",
        b"new",
        0o644,
        Action::Replace,
    );
    apply(
        &env.platform,
        &plan(vec![a], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    let jp = env.session_dir.join("journal.cbor");
    let mut bytes = std::fs::read(&jp).unwrap();
    bytes.extend_from_slice(&100u32.to_le_bytes());
    bytes.extend_from_slice(&[0xa1, 0x67]);
    std::fs::write(&jp, &bytes).unwrap();
    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty());
    assert_eq!(env.snap(), before);
    // The journal is valid again afterwards.
    assert_eq!(
        journal::read(&env.platform, &jp).unwrap().last(),
        Some(&Record::RolledBack)
    );
}

#[test]
fn rollback_converges_after_partial_manual_undo() {
    // Simulates a crash mid-rollback: some steps already undone on disk
    // but not marked in the journal.
    let mut env = Env::new();
    seed_home(&env);
    let before = env.snap();
    let a = file(
        &mut env,
        "a",
        0,
        Home,
        "notes.txt",
        b"new",
        0o644,
        Action::Replace,
    );
    let b = file(
        &mut env,
        "a",
        1,
        Home,
        "dir/new.txt",
        b"n",
        0o644,
        Action::Create,
    );
    apply(
        &env.platform,
        &plan(vec![a, b], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    std::fs::remove_file(env.home.join("dir/new.txt")).unwrap();
    std::fs::remove_file(env.home.join("notes.txt")).unwrap();
    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty(), "{:?}", rb.failed);
    assert_eq!(env.snap(), before);
}

#[test]
fn rollback_reports_foreign_changes_instead_of_deleting_them() {
    let mut env = Env::new();
    let a = file(
        &mut env,
        "a",
        0,
        Home,
        "d/new.txt",
        b"n",
        0o644,
        Action::Create,
    );
    apply(
        &env.platform,
        &plan(vec![a], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    // The user added a file to the dir apply created.
    std::fs::write(env.home.join("d/mine.txt"), b"user data").unwrap();
    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert_eq!(rb.failed.len(), 1);
    assert_eq!(
        std::fs::read(env.home.join("d/mine.txt")).unwrap(),
        b"user data"
    );
    assert!(!env.home.join("d/new.txt").exists());
    // Once resolved, a re-run completes.
    std::fs::remove_file(env.home.join("d/mine.txt")).unwrap();
    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty());
    assert!(env.snap().is_empty());
}

#[test]
fn dismiss_removes_only_journal_and_backup() {
    let mut env = Env::new();
    seed_home(&env);
    std::fs::write(env.session_dir.join("session.json"), b"{}").unwrap();
    let a = file(
        &mut env,
        "a",
        0,
        Home,
        "notes.txt",
        b"new",
        0o644,
        Action::Replace,
    );
    apply(
        &env.platform,
        &plan(vec![a], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    assert!(env.session_dir.join("backup/1").exists());
    let after = env.snap();
    dismiss(&env.platform, &env.session_dir).unwrap();
    assert!(!env.session_dir.join("backup").exists());
    assert!(!env.session_dir.join("journal.cbor").exists());
    assert!(env.session_dir.join("session.json").exists());
    assert!(env.staging.exists());
    assert_eq!(env.snap(), after);
    // Idempotent; rollback is no longer possible.
    dismiss(&env.platform, &env.session_dir).unwrap();
    assert!(rollback(&env.platform, &env.session_dir, &NullSink).is_err());
}
