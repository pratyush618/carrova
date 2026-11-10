//! Apply never loosens permissions, refuses symlinked roots, and rollback
//! never destroys data changed or created after the apply.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/apply.rs"]
mod apply_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use apply_support::{Env, action, file, plan, seed_home};
use carrova_core::CoreError;
use carrova_core::plan::{Action, ObjectKind};
use carrova_core::session::{Event, NullSink, RollbackOutcome, VecSink};
use carrova_core::test_support::apply::{Failpoint, apply};
use carrova_core::test_support::rollback::rollback;
use carrova_platform::Platform;
use carrova_platform::SemanticRoot::{Documents, Home};

const NO_FAIL: Failpoint = Failpoint::OFF;

#[test]
fn group_and_world_write_bits_are_masked_on_new_objects() {
    let mut env = Env::new();
    let f = file(
        &mut env,
        "a",
        0,
        Home,
        "wide/file",
        b"x",
        0o777,
        Action::Create,
    );
    let d = action(
        &env,
        "a",
        1,
        ObjectKind::Dir,
        Home,
        "wide",
        0,
        Some(0o777),
        Action::Create,
    );
    apply(
        &env.platform,
        &plan(vec![f, d], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    assert_eq!(env.mode(&env.home.join("wide")), 0o755);
    assert_eq!(env.mode(&env.home.join("wide/file")), 0o755);
}

/// Applies 0o664 / 0o666 files and an incoming 0o775 over an existing
/// 0o755 dir; returns the resulting `(file 664, file 666, dir)` modes.
fn apply_group_writable(keep_group_write: bool) -> (u32, u32, u32) {
    let mut env = Env::new();
    std::fs::create_dir_all(env.home.join("shared")).unwrap();
    env.platform
        .set_mode(&env.home.join("shared"), 0o755)
        .unwrap();
    let a = file(&mut env, "a", 0, Home, "gw/a", b"a", 0o664, Action::Create);
    let b = file(&mut env, "a", 1, Home, "gw/b", b"b", 0o666, Action::Create);
    let d = action(
        &env,
        "a",
        2,
        ObjectKind::Dir,
        Home,
        "shared",
        0,
        Some(0o775),
        Action::Replace,
    );
    let mut p = plan(vec![a, b, d], vec![]);
    p.options.keep_group_write = keep_group_write;
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
    (
        env.mode(&env.home.join("gw/a")),
        env.mode(&env.home.join("gw/b")),
        env.mode(&env.home.join("shared")),
    )
}

#[test]
fn group_write_dropped_unless_plan_keeps_it() {
    assert_eq!(apply_group_writable(false), (0o644, 0o644, 0o755));
    // World-write still dropped; an existing dir is still never loosened.
    assert_eq!(apply_group_writable(true), (0o664, 0o664, 0o755));
}

#[test]
fn existing_dir_modes_only_lose_bits() {
    let env = Env::new();
    seed_home(&env); // .ssh is 700, .config/app is 750
    let ssh = action(
        &env,
        "ssh",
        0,
        ObjectKind::Dir,
        Home,
        ".ssh",
        0,
        Some(0o777),
        Action::Replace,
    );
    let app = action(
        &env,
        "app",
        0,
        ObjectKind::Dir,
        carrova_platform::SemanticRoot::UserConfig,
        "app",
        0,
        Some(0o705),
        Action::Replace,
    );
    apply(
        &env.platform,
        &plan(vec![ssh, app], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    // 700 & 777 → unchanged; 750 & 705 → 700.
    assert_eq!(env.mode(&env.home.join(".ssh")), 0o700);
    assert_eq!(env.mode(&env.home.join(".config/app")), 0o700);
    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty(), "{:?}", rb.failed);
    assert_eq!(env.mode(&env.home.join(".config/app")), 0o750);
}

#[test]
fn symlinked_root_is_refused() {
    let mut env = Env::new();
    let real = env.tmp.path().join("real-docs");
    std::fs::create_dir(&real).unwrap();
    carrova_platform::fake::symlink(&real, env.home.join("Documents")).unwrap();
    let a = file(
        &mut env,
        "d",
        0,
        Documents,
        "x.txt",
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
    match err {
        CoreError::RolledBack { cause } => assert!(cause.contains("symlink"), "{cause}"),
        e => panic!("unexpected {e}"),
    }
    assert_eq!(std::fs::read_dir(&real).unwrap().count(), 0);
}

#[test]
fn rollback_leaves_created_file_edited_after_apply() {
    let mut env = Env::new();
    let a = file(
        &mut env,
        "a",
        0,
        Home,
        "conf.txt",
        b"imported",
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
    let p = env.home.join("conf.txt");
    // Same size, different bytes: the content check must catch it.
    std::fs::write(&p, b"IMPORTED").unwrap();
    let sink = VecSink::default();
    let rb = rollback(&env.platform, &env.session_dir, &sink).unwrap();
    assert_eq!(rb.failed.len(), 1, "{rb:?}");
    assert!(rb.failed[0].target.contains("conf.txt"));
    assert_eq!(
        sink.events(),
        [Event::RollbackStep {
            target: rb.failed[0].target.clone(),
            outcome: RollbackOutcome::Failed,
        }],
        "a failed step is reported as failed, not as removed"
    );
    assert_eq!(std::fs::read(&p).unwrap(), b"IMPORTED");
    assert!(rb.removed.is_empty());
}

#[test]
fn rollback_keeps_backup_when_replaced_file_was_edited() {
    let mut env = Env::new();
    seed_home(&env);
    let a = file(
        &mut env,
        "a",
        0,
        Home,
        "notes.txt",
        b"imported notes",
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
    let p = env.home.join("notes.txt");
    std::fs::write(&p, b"user edited later").unwrap();
    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert!(!rb.failed.is_empty(), "{rb:?}");
    assert_eq!(std::fs::read(&p).unwrap(), b"user edited later");
    // The original is still in the rollback store.
    assert_eq!(
        std::fs::read(env.session_dir.join("backup/1")).unwrap(),
        b"old notes"
    );
    // Untouched file: rollback still works once the user resolves it.
    std::fs::remove_file(&p).unwrap();
    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty(), "{:?}", rb.failed);
    assert_eq!(std::fs::read(&p).unwrap(), b"old notes");
    assert_eq!(env.mode(&p), 0o644);
}
