//! Handler runs during apply: declared paths are backed up first and
//! restored on rollback. Uses a fake `gpg` (shell script) so the handler's
//! mutations are deterministic.
//!
//! One test function on purpose: scripts are written then executed, and a
//! concurrent fork in another test thread could hold the write fd and make
//! exec fail with ETXTBSY.
// Fake `gpg` is a `#!/bin/sh` script.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/apply.rs"]
mod apply_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::path::{Path, PathBuf};

use apply_support::{Env, file, plan};
use carrova_core::plan::{Action, PlannedHandler};
use carrova_core::session::NullSink;
use carrova_core::test_support::apply::{Failpoint, apply};
use carrova_core::test_support::handlers;
use carrova_core::test_support::rollback::rollback;
use carrova_core::{CoreError, Quarantined};
use carrova_platform::Platform;
use carrova_platform::SemanticRoot::Home;

fn script(dir: &Path, name: &str, exit: u8, junk: bool) -> PathBuf {
    let calls = dir.join(format!("{name}.calls"));
    let junk = if junk {
        "printf j > \"$GNUPGHOME/random_seed\""
    } else {
        ""
    };
    let body = format!(
        "printf '%s\\n' \"$*\" >> '{calls}'\n\
         case \"$*\" in\n\
         *--import-ownertrust*) printf T > \"$GNUPGHOME/trustdb.gpg\" ;;\n\
         *--import*) printf MUTATED > \"$GNUPGHOME/pubring.kbx\"\n\
            mkdir -p \"$GNUPGHOME/public-keys.d\"\n\
            printf x > \"$GNUPGHOME/public-keys.d/new\"\n\
            {junk} ;;\n\
         esac\n\
         exit {exit}\n",
        calls = calls.display()
    );
    carrova_platform::fake::write_tool_script(dir, name, &body).unwrap()
}

fn gpg_handler() -> PlannedHandler {
    PlannedHandler {
        item: "gnupg/public-keyring".into(),
        handler: carrova_core::plan::Handler::GpgKeyring,
        backup: vec![
            (Home, ".gnupg/pubring.kbx".into()),
            (Home, ".gnupg/trustdb.gpg".into()),
            (Home, ".gnupg/public-keys.d".into()),
        ],
        import_trust: true,
    }
}

fn seed_gnupg(env: &Env) {
    let g = env.home.join(".gnupg");
    std::fs::create_dir(&g).unwrap();
    env.platform.set_mode(&g, 0o700).unwrap();
    env.put(&g.join("pubring.kbx"), b"ORIGINAL-KEYRING", 0o600);
    env.put(&g.join("public-keys.d/pubring.db"), b"db", 0o600);
    env.platform
        .set_mode(&g.join("public-keys.d"), 0o700)
        .unwrap();
    env.put(&g.join("gpg.conf"), b"old conf", 0o600);
}

fn with_artifacts(env: &mut Env) {
    env.stage_artifact(
        "gnupg/public-keyring",
        "pubring.asc",
        b"-----BEGIN PGP-----",
    );
    env.stage_artifact("gnupg/public-keyring", "ownertrust.txt", b"ABC:6:\n");
}

#[test]
fn handler_backups_and_rollback() {
    let bin = tempfile::TempDir::new().unwrap();
    let ok = script(bin.path(), "gpg-ok", 0, false);
    let ok_junk = script(bin.path(), "gpg-junk", 0, true);
    let failing = script(bin.path(), "gpg-fail", 1, false);

    // 1. gpg missing: apply fails before the handler runs; file actions
    //    and backups are rolled back.
    {
        let mut env = Env::new();
        seed_gnupg(&env);
        with_artifacts(&mut env);
        let before = env.snap();
        let conf = file(
            &mut env,
            "gnupg/config",
            0,
            Home,
            ".gnupg/gpg.conf",
            b"new conf",
            0o600,
            Action::Replace,
        );
        let err = apply(
            &env.platform,
            &plan(vec![conf], vec![gpg_handler()]),
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::OFF,
        )
        .unwrap_err();
        match err {
            CoreError::RolledBack { cause } => {
                assert!(cause.contains("gpg is not installed"), "{cause}");
            }
            e => panic!("unexpected {e}"),
        }
        assert_eq!(env.snap(), before);
    }

    // 2. Handler mutates the keyring then fails → declared paths restored,
    //    created ones (trustdb absent before) removed.
    {
        let mut env = Env::with(|p| p.binary("gpg", &failing));
        seed_gnupg(&env);
        with_artifacts(&mut env);
        let before = env.snap();
        let err = apply(
            &env.platform,
            &plan(vec![], vec![gpg_handler()]),
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::OFF,
        )
        .unwrap_err();
        match err {
            CoreError::RolledBack { cause } => {
                assert!(cause.contains("handler gpg-keyring failed"), "{cause}");
            }
            e => panic!("unexpected {e}"),
        }
        assert_eq!(env.snap(), before);
    }

    // 3. Handler succeeds; manual rollback restores originals exactly.
    {
        let mut env = Env::with(|p| p.binary("gpg", &ok));
        seed_gnupg(&env);
        with_artifacts(&mut env);
        let before = env.snap();
        let r = apply(
            &env.platform,
            &plan(vec![], vec![gpg_handler()]),
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::OFF,
        )
        .unwrap();
        assert_eq!(r.handlers_run, ["gnupg/public-keyring"]);
        let g = env.home.join(".gnupg");
        assert_eq!(std::fs::read(g.join("pubring.kbx")).unwrap(), b"MUTATED");
        assert_eq!(std::fs::read(g.join("trustdb.gpg")).unwrap(), b"T");
        let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
        assert!(rb.failed.is_empty(), "{:?}", rb.failed);
        assert_eq!(env.snap(), before);
        let again = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
        assert_eq!(again, Default::default());
        assert_eq!(env.snap(), before);
    }

    // 4. Fresh home: ~/.gnupg is created 700 for the handler; on rollback
    //    the declared outputs are removed and the rest (undeclared files the
    //    tool dropped) is quarantined with the directory, never deleted.
    {
        let mut env = Env::with(|p| p.binary("gpg", &ok_junk));
        with_artifacts(&mut env);
        apply(
            &env.platform,
            &plan(vec![], vec![gpg_handler()]),
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::OFF,
        )
        .unwrap();
        let g = env.home.join(".gnupg");
        assert_eq!(env.mode(&g), 0o700);
        assert!(g.join("random_seed").exists());
        let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
        assert!(rb.failed.is_empty(), "{:?}", rb.failed);
        assert!(env.snap().is_empty(), "{:?}", env.snap());
        assert_eq!(rb.quarantined.len(), 1, "{rb:?}");
        let moved = &rb.quarantined[0].moved_to;
        let left: Vec<_> = std::fs::read_dir(moved)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["random_seed"]);
    }

    // 4b. File actions inside a handler dir apply created are undone (by
    //     hash) before the dir is judged; an edited one blocks the dir.
    for edit in [false, true] {
        let mut env = Env::with(|p| p.binary("gpg", &ok));
        with_artifacts(&mut env);
        let key = file(
            &mut env,
            "gnupg/private-keys",
            0,
            Home,
            ".gnupg/private-keys-v1.d/K.key",
            b"KEY",
            0o600,
            Action::Create,
        );
        apply(
            &env.platform,
            &plan(vec![key], vec![gpg_handler()]),
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::OFF,
        )
        .unwrap();
        let k = env.home.join(".gnupg/private-keys-v1.d/K.key");
        if edit {
            std::fs::write(&k, b"NEW").unwrap();
        }
        let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
        assert!(rb.quarantined.is_empty(), "{rb:?}");
        if edit {
            assert_eq!(std::fs::read(&k).unwrap(), b"NEW");
            assert!(
                rb.failed.iter().any(|f| f.target.ends_with("K.key")),
                "{rb:?}"
            );
            assert!(
                rb.failed.iter().any(|f| f.target.ends_with(".gnupg")),
                "{rb:?}"
            );
        } else {
            assert!(rb.failed.is_empty(), "{:?}", rb.failed);
            assert!(env.snap().is_empty(), "{:?}", env.snap());
        }
    }

    // 6. Rollback weeks later: data the user created after the import in
    //    handler-owned dirs is never lost (quarantined, not deleted).
    {
        // 6a. Existing ~/.gnupg: a new key file in the backed-up
        //     public-keys.d survives.
        let mut env = Env::with(|p| p.binary("gpg", &ok));
        seed_gnupg(&env);
        with_artifacts(&mut env);
        apply(
            &env.platform,
            &plan(vec![], vec![gpg_handler()]),
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::OFF,
        )
        .unwrap();
        let g = env.home.join(".gnupg");
        env.put(&g.join("public-keys.d/later.key"), b"USER KEY", 0o600);
        let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
        assert!(rb.failed.is_empty(), "{:?}", rb.failed);
        assert_eq!(rb.quarantined.len(), 1, "{rb:?}");
        let Quarantined {
            target: orig,
            moved_to: moved,
        } = &rb.quarantined[0];
        assert!(orig.ends_with("public-keys.d"), "{orig}");
        assert!(moved.starts_with(env.session_dir.join("rollback-trash")));
        assert_eq!(std::fs::read(moved.join("later.key")).unwrap(), b"USER KEY");
        // Original public-keys.d restored.
        assert_eq!(
            std::fs::read(g.join("public-keys.d/pubring.db")).unwrap(),
            b"db"
        );
        assert!(!g.join("public-keys.d/later.key").exists());

        // 6b. Fresh home: ~/.gnupg was created by apply; the user then put
        //     a private key there. Rollback must not delete it.
        let mut env = Env::with(|p| p.binary("gpg", &ok));
        with_artifacts(&mut env);
        apply(
            &env.platform,
            &plan(vec![], vec![gpg_handler()]),
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::OFF,
        )
        .unwrap();
        let g = env.home.join(".gnupg");
        env.put(&g.join("private-keys-v1.d/ABC.key"), b"SECRET", 0o600);
        let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
        assert!(rb.failed.is_empty(), "{:?}", rb.failed);
        assert!(env.snap().is_empty(), "{:?}", env.snap());
        let Quarantined {
            target: orig,
            moved_to: moved,
        } = rb.quarantined.last().unwrap();
        assert!(orig.ends_with(".gnupg"), "{orig}");
        assert_eq!(
            std::fs::read(moved.join("private-keys-v1.d/ABC.key")).unwrap(),
            b"SECRET"
        );

        // 6c. A handler-written file the user changed later is left in
        //     place and reported; its backup is kept.
        let mut env = Env::with(|p| p.binary("gpg", &ok));
        seed_gnupg(&env);
        with_artifacts(&mut env);
        apply(
            &env.platform,
            &plan(vec![], vec![gpg_handler()]),
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::OFF,
        )
        .unwrap();
        let ring = env.home.join(".gnupg/pubring.kbx");
        std::fs::write(&ring, b"USER IMPORTED MORE").unwrap();
        let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
        assert!(
            rb.failed.iter().any(|f| f.target.contains("pubring.kbx")),
            "{rb:?}"
        );
        assert_eq!(std::fs::read(&ring).unwrap(), b"USER IMPORTED MORE");
    }

    // 7. Ownertrust is imported only when the user opted in.
    for trust in [false, true] {
        let bin = tempfile::TempDir::new().unwrap();
        let gpg = script(bin.path(), "gpg", 0, false);
        let mut env = Env::with(|p| p.binary("gpg", &gpg));
        with_artifacts(&mut env);
        let mut h = gpg_handler();
        h.import_trust = trust;
        apply(
            &env.platform,
            &plan(vec![], vec![h]),
            &env.manifest(),
            &env.staged,
            &env.session_dir,
            &NullSink,
            Failpoint::OFF,
        )
        .unwrap();
        let calls = std::fs::read_to_string(bin.path().join("gpg.calls")).unwrap();
        assert!(calls.contains("--import "), "{calls}");
        assert_eq!(
            calls.contains("--import-ownertrust"),
            trust,
            "trust={trust}: {calls}"
        );
        assert_eq!(env.home.join(".gnupg/trustdb.gpg").exists(), trust);
    }

    // 5. Unknown handler names are refused: they cannot enter a plan
    //    (a tampered plan.json does not load) nor be exported.
    {
        let env = Env::new();
        let mut json = serde_json::to_value(gpg_handler()).unwrap();
        json["handler"] = "rm-rf".into();
        assert!(serde_json::from_value::<PlannedHandler>(json).is_err());
        assert!(!handlers::is_known("rm-rf"));
        assert!(handlers::is_known("gpg-keyring"));
        assert!(handlers::export(&env.platform, "rm-rf").is_err());
    }
}
