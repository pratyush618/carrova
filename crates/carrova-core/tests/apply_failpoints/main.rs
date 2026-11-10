//! Injected failures after k actions roll back to a byte-identical tree.
//! Run with `--features failpoints`.
#![cfg(feature = "failpoints")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/apply.rs"]
mod apply_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use apply_support::{Env, action, file, plan, seed_home};
use carrova_core::CoreError;
use carrova_core::plan::{Action, ObjectKind, Plan};
use carrova_core::session::NullSink;
use carrova_core::test_support::apply::{Failpoint, apply};
use carrova_platform::SemanticRoot::{Home, UserConfig};

fn mixed(env: &mut Env) -> Plan {
    let actions = vec![
        action(
            env,
            "ssh",
            0,
            ObjectKind::Dir,
            Home,
            ".ssh",
            0,
            Some(0o700),
            Action::Replace,
        ),
        file(
            env,
            "ssh",
            1,
            Home,
            ".ssh/id_ed25519",
            b"NEW-KEY",
            0o600,
            Action::Replace,
        ),
        file(
            env,
            "ssh",
            2,
            Home,
            ".ssh/config",
            b"Host *",
            0o644,
            Action::Create,
        ),
        file(
            env,
            "app",
            0,
            UserConfig,
            "app/settings.json",
            b"{}",
            0o600,
            Action::Replace,
        ),
        file(
            env,
            "app",
            1,
            UserConfig,
            "app/deep/er/x.bin",
            &[0u8; 4096],
            0o640,
            Action::Create,
        ),
        file(
            env,
            "doc",
            0,
            Home,
            "notes.txt",
            b"kb",
            0o644,
            Action::KeepBoth {
                rel: "notes.carrova.txt".into(),
            },
        ),
        file(
            env,
            "doc",
            1,
            Home,
            "skipped",
            b"s",
            0o644,
            Action::Skip {
                reason: "user".into(),
            },
        ),
        file(
            env,
            "doc",
            2,
            Home,
            "brand/new/file",
            b"fresh",
            0o600,
            Action::Create,
        ),
    ];
    plan(actions, vec![])
}

#[test]
fn failure_after_k_actions_restores_tree() {
    let writes = 7; // non-skip actions in `mixed`
    for k in [0, 1, 3, writes - 1, writes] {
        let mut env = Env::new();
        seed_home(&env);
        let before = env.snap();
        let p = mixed(&mut env);
        let err = apply(
            &env.platform,
            &p,
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::after(k),
        )
        .unwrap_err();
        match err {
            CoreError::RolledBack { cause } => {
                assert!(cause.contains("injected"), "k={k}: {cause}");
            }
            e => panic!("k={k}: unexpected {e}"),
        }
        assert_eq!(env.snap(), before, "tree differs after rollback at k={k}");
        // Staging only ever loses files (moved into place, then removed by
        // rollback); nothing new is left there (staging wipe is the
        // engine's job).
        let staged: Vec<_> = env.staged.files.values().cloned().collect();
        for entry in std::fs::read_dir(&env.staging).unwrap() {
            let path = entry.unwrap().path();
            assert!(staged.contains(&path), "k={k}: unexpected {path:?}");
        }
    }
}

#[test]
fn failpoint_beyond_action_count_does_not_fire() {
    let mut env = Env::new();
    seed_home(&env);
    let p = mixed(&mut env);
    let r = apply(
        &env.platform,
        &p,
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        Failpoint::after(100),
    )
    .unwrap();
    assert_eq!(r.written.len(), 7);
    assert_eq!(r.backed_up.len(), 2);
}
