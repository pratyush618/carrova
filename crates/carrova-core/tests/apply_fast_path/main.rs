//! Apply fast path: staged files renamed into place in journalled batches,
//! cross-device copy fallback, and rollback of journals written by it (and
//! by the previous format).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/apply.rs"]
mod apply_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use apply_support::{Env, SID, file, plan, seed_home};
use carrova_core::CoreError;
use carrova_core::plan::Action;
use carrova_core::session::NullSink;
use carrova_core::test_support::apply::{APPLY_BATCH, Failpoint, apply};
use carrova_core::test_support::journal::{self, BackupKind, JOURNAL_FILE, Journal, Record};
use carrova_core::test_support::rollback::rollback;
use carrova_manifest::Blake3Hash;
use carrova_platform::Platform;
use carrova_platform::SemanticRoot::Home;
use carrova_platform::fake::Hooks;
use common::{cross_device_from, fail_rename_once};
use serde::Serialize;

const NO_FAIL: Failpoint = Failpoint::OFF;
const MTIME_NS: i64 = 1_700_000_000_123_456_789;

fn expected_mtime() -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_nanos(MTIME_NS.unsigned_abs())
}

fn staged_path(env: &Env, item: &str, idx: usize) -> PathBuf {
    env.staged.files[&(item.into(), idx)].clone()
}

fn no_temp_files(env: &Env) {
    for (path, _) in env.snap() {
        assert!(!path.to_string_lossy().contains("carrova-tmp"), "{path:?}");
    }
}

#[test]
fn staged_file_is_moved_into_place_with_exact_mode_and_mtime() {
    let mut env = Env::new();
    let mut a = file(
        &mut env,
        "a",
        0,
        Home,
        "cfg/a.conf",
        b"alpha",
        0o640,
        Action::Create,
    );
    a.mtime_ns = Some(MTIME_NS);
    let mut b = file(
        &mut env,
        "a",
        1,
        Home,
        "cfg/ro",
        b"read-only",
        0o400,
        Action::Create,
    );
    b.mtime_ns = Some(MTIME_NS);
    let staged = [staged_path(&env, "a", 0), staged_path(&env, "a", 1)];
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

    // Moved, not copied: the staged files are gone.
    for s in &staged {
        assert!(!s.exists(), "{s:?} still staged");
    }
    let (ta, tb) = (env.home.join("cfg/a.conf"), env.home.join("cfg/ro"));
    assert_eq!(std::fs::read(&ta).unwrap(), b"alpha");
    assert_eq!(std::fs::read(&tb).unwrap(), b"read-only");
    assert_eq!(env.mode(&ta), 0o640);
    assert_eq!(env.mode(&tb), 0o400);
    for t in [&ta, &tb] {
        let m = env.platform.metadata(t).unwrap();
        assert_eq!(m.modified, Some(expected_mtime()), "{t:?}");
    }
    no_temp_files(&env);

    // Rollback recognises the moved files by the manifest hash.
    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty(), "{:?}", rb.failed);
    assert!(env.snap().is_empty());
}

#[test]
fn cross_device_staging_falls_back_to_copy() {
    let mut env = Env::new();
    seed_home(&env);
    let before = env.snap();
    let mut a = file(
        &mut env,
        "a",
        0,
        Home,
        "new/a.txt",
        b"alpha",
        0o644,
        Action::Create,
    );
    a.mtime_ns = Some(MTIME_NS);
    let mut r = file(
        &mut env,
        "a",
        1,
        Home,
        "notes.txt",
        b"new notes",
        0o400,
        Action::Replace,
    );
    r.mtime_ns = Some(MTIME_NS);
    let hooked = env.platform.clone().with_hooks(Hooks {
        on_rename: Some(cross_device_from(env.staging.clone())),
        ..Hooks::default()
    });
    apply(
        &hooked,
        &plan(vec![a, r], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();

    let (ta, tr) = (env.home.join("new/a.txt"), env.home.join("notes.txt"));
    assert_eq!(std::fs::read(&ta).unwrap(), b"alpha");
    assert_eq!(std::fs::read(&tr).unwrap(), b"new notes");
    assert_eq!(env.mode(&ta), 0o644);
    assert_eq!(env.mode(&tr), 0o400);
    for t in [&ta, &tr] {
        let m = env.platform.metadata(t).unwrap();
        assert_eq!(m.modified, Some(expected_mtime()), "{t:?}");
    }
    // Copied: the staged files are still there; no temp left behind.
    assert!(staged_path(&env, "a", 0).exists());
    assert!(staged_path(&env, "a", 1).exists());
    no_temp_files(&env);
    let recs = journal::read(&env.platform, &env.session_dir.join(JOURNAL_FILE)).unwrap();
    let temps = recs
        .iter()
        .filter(|r| matches!(r, Record::CreatedTemp { .. }))
        .count();
    assert_eq!(temps, 2);

    let rb = rollback(&hooked, &env.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty(), "{:?}", rb.failed);
    assert_eq!(env.snap(), before);
}

#[test]
fn cross_device_fallback_reads_staged_file_whose_final_mode_is_write_only() {
    let mut env = Env::new();
    let a = file(
        &mut env,
        "a",
        0,
        Home,
        "wo",
        b"secret-ish",
        0o200,
        Action::Create,
    );
    let hooked = env.platform.clone().with_hooks(Hooks {
        on_rename: Some(cross_device_from(env.staging.clone())),
        ..Hooks::default()
    });
    apply(
        &hooked,
        &plan(vec![a], vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    let t = env.home.join("wo");
    assert_eq!(env.mode(&t), 0o200);
    assert_eq!(env.platform.metadata(&t).unwrap().size, 10);
}

#[test]
fn rename_failure_mid_batch_rolls_back_every_file() {
    let mut env = Env::new();
    seed_home(&env);
    let before = env.snap();
    let actions = vec![
        file(&mut env, "a", 0, Home, "x/one", b"1", 0o644, Action::Create),
        file(
            &mut env,
            "a",
            1,
            Home,
            "notes.txt",
            b"N",
            0o644,
            Action::Replace,
        ),
        file(
            &mut env,
            "a",
            2,
            Home,
            ".ssh/id_ed25519",
            b"NEW-KEY",
            0o600,
            Action::Replace,
        ),
        file(&mut env, "a", 3, Home, "x/two", b"2", 0o644, Action::Create),
    ];
    // The third file's backup is taken, then its rename fails; the fourth
    // file's intent is journalled but never executed.
    let hooked = env.platform.clone().with_hooks(Hooks {
        on_rename: Some(fail_rename_once(env.home.join(".ssh/id_ed25519"))),
        ..Hooks::default()
    });
    let err = apply(
        &hooked,
        &plan(actions, vec![]),
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
fn rollback_after_crash_before_rename_keeps_original_when_staging_is_gone() {
    // Crash after the batch's intents became durable, before any step ran;
    // staging was wiped since. The original happens to have the incoming
    // bytes (only the mode differs), so the hash alone cannot tell it apart
    // from what apply would have written.
    let env = Env::new();
    seed_home(&env);
    let before = env.snap();
    let abs = env.home.join("notes.txt");
    let backup = env.session_dir.join("backup/1");
    let mut j = Journal::create(&env.platform, &env.session_dir.join(JOURNAL_FILE)).unwrap();
    j.append(&Record::Begin {
        session: SID.into(),
        version: journal::JOURNAL_VERSION,
    })
    .unwrap();
    j.push(&Record::BackedUp {
        abs: abs.clone(),
        backup: backup.clone(),
        kind: BackupKind::File,
        mode: Some(0o644),
    })
    .unwrap();
    j.push(&Record::CreatedFile {
        abs: abs.clone(),
        src: env.staging.join("files/0"),
        hash: Blake3Hash::of(b"old notes"),
        size: 9,
        backup: Some(backup),
    })
    .unwrap();
    j.sync().unwrap();
    drop(j);

    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty(), "{:?}", rb.failed);
    assert!(rb.removed.is_empty(), "{:?}", rb.removed);
    assert_eq!(env.snap(), before);
}

/// Journal records exactly as the previous format (v2) wrote them.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum RecordV2 {
    Begin {
        session: String,
        version: u8,
    },
    CreatedDir {
        abs: PathBuf,
    },
    CreatedTemp {
        abs: PathBuf,
    },
    CreatedFile {
        abs: PathBuf,
        tmp: PathBuf,
        hash: Blake3Hash,
        size: u64,
    },
    BackedUp {
        abs: PathBuf,
        backup: PathBuf,
        kind: BackupKind,
        mode: Option<u32>,
    },
    Committed,
}

fn write_v2_journal(path: &Path, records: &[RecordV2]) {
    let mut bytes = Vec::new();
    for r in records {
        let mut body = Vec::new();
        ciborium::into_writer(r, &mut body).unwrap();
        bytes.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&body);
    }
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn journal_from_previous_format_still_rolls_back() {
    let env = Env::new();
    seed_home(&env);
    let before = env.snap();
    // Disk state after a v2 apply: notes.txt replaced (original in the
    // backup store), d/new.txt created in a new directory.
    let notes = env.home.join("notes.txt");
    let backup_dir = env.session_dir.join("backup");
    std::fs::create_dir(&backup_dir).unwrap();
    let backup = backup_dir.join("1");
    std::fs::rename(&notes, &backup).unwrap();
    env.put(&notes, b"imported notes", 0o600);
    let dir = env.home.join("d");
    std::fs::create_dir(&dir).unwrap();
    let created = dir.join("new.txt");
    env.put(&created, b"fresh", 0o644);
    let tmp = |abs: &Path| {
        let name = abs.file_name().unwrap().to_string_lossy().into_owned();
        abs.with_file_name(format!(".{name}.carrova-tmp-{SID}"))
    };
    write_v2_journal(
        &env.session_dir.join(JOURNAL_FILE),
        &[
            RecordV2::Begin {
                session: SID.into(),
                version: 2,
            },
            RecordV2::CreatedTemp { abs: tmp(&notes) },
            RecordV2::BackedUp {
                abs: notes.clone(),
                backup,
                kind: BackupKind::File,
                mode: Some(0o644),
            },
            RecordV2::CreatedFile {
                abs: notes.clone(),
                tmp: tmp(&notes),
                hash: Blake3Hash::of(b"imported notes"),
                size: 14,
            },
            RecordV2::CreatedDir { abs: dir.clone() },
            RecordV2::CreatedTemp { abs: tmp(&created) },
            RecordV2::CreatedFile {
                abs: created.clone(),
                tmp: tmp(&created),
                hash: Blake3Hash::of(b"fresh"),
                size: 5,
            },
            RecordV2::Committed,
        ],
    );

    let rb = rollback(&env.platform, &env.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty(), "{:?}", rb.failed);
    assert_eq!(env.snap(), before);
    let recs = journal::read(&env.platform, &env.session_dir.join(JOURNAL_FILE)).unwrap();
    assert_eq!(recs.last(), Some(&Record::RolledBack));
}

#[test]
fn directories_are_synced_once_per_batch() {
    let mut env = Env::new();
    for d in ["a", "b"] {
        std::fs::create_dir(env.home.join(d)).unwrap();
    }
    let files = APPLY_BATCH + 2; // two batches, both touching `a` and `b`
    let actions = (0..files)
        .map(|i| {
            let rel = format!("{}/f{i}", if i % 2 == 0 { "a" } else { "b" });
            file(&mut env, "a", i, Home, &rel, b"x", 0o644, Action::Create)
        })
        .collect();
    let hooked = env.platform.clone().with_hooks(Hooks::default());
    apply(
        &hooked,
        &plan(actions, vec![]),
        &env.manifest(),
        &env.staged,
        &env.session_dir,
        &NullSink,
        NO_FAIL,
    )
    .unwrap();
    // Per batch: `a`, `b` and the staging directory.
    assert_eq!(hooked.dir_syncs(), 2 * 3);
    assert_eq!(
        std::fs::read_dir(env.home.join("a")).unwrap().count(),
        files.div_ceil(2)
    );
}
