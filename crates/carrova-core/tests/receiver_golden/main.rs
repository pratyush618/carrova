//! Golden guards for the receive pipeline: what is persisted or shown must
//! stay byte-identical across refactors — plan JSON (incl. every blocker,
//! warning and skip text), journal CBOR encodings, apply events + journal,
//! rollback outcomes, failure reasons and events.
//!
//! Fixtures live in `tests/receiver_golden/golden/`. Regenerate (only for an intended
//! format change) with `CARROVA_BLESS=1 cargo test -p carrova-core --test
//! receiver_golden`.
// Goldens hold Unix modes and `/`-separated absolute paths.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::path::{Path, PathBuf};

use carrova_core::plan::{ConflictRule, Plan, PlanOptions, Remap, plan_on_date};
use carrova_core::session::{SessionId, VecSink};
use carrova_core::test_support::apply::{Failpoint, apply};
use carrova_core::test_support::journal::{
    self, BackupKind, Fingerprint, JOURNAL_FILE, Journal, Record,
};
use carrova_core::test_support::rollback::rollback;
use carrova_core::test_support::stage::Staged;
use carrova_manifest::{
    Artifact, Blake3Hash, DEFAULT_CHUNK_PARAMS, Entry, ItemFlags, Manifest, ManifestItem,
    SourceInfo, chunk_reader, entry_from_path,
};
use carrova_platform::fake::{FakePlatform, pin_umask, symlink};
use carrova_platform::{OsKind, Platform, SemanticRoot};
use carrova_recipe::Recipe;
use common::{mkdir, put};
use tempfile::TempDir;

const DATE: &str = "20261108";
const MTIME_NS: i64 = 1_700_000_000_123_456_789;

// ---------------------------------------------------------------- golden

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/receiver_golden/golden")
        .join(name)
}

/// Compare `actual` with the checked-in fixture (`CARROVA_BLESS=1` rewrites it).
fn golden(name: &str, actual: &str) {
    let path = golden_path(name);
    if std::env::var_os("CARROVA_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    assert!(
        expected == actual,
        "{name} differs from its golden fixture\n--- expected\n{expected}\n--- actual\n{actual}"
    );
}

/// `text` with the temp dir replaced by `$TMP` (paths are machine-specific).
fn scrub(text: &str, tmp: &Path) -> String {
    text.replace(tmp.to_str().unwrap(), "$TMP")
}

fn json<T: serde::Serialize>(v: &T) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap();
    s.push('\n');
    s
}

// ---------------------------------------------------------------- env

struct Env {
    tmp: TempDir,
    sender_home: PathBuf,
    recv_home: PathBuf,
    sender: FakePlatform,
    recv: FakePlatform,
}

fn env() -> Env {
    // Goldens were recorded under umask 002; pin it so implicit dirs and
    // (on macOS) symlink modes match on any host.
    pin_umask(0o002);
    let tmp = TempDir::new().unwrap();
    let sender_home = tmp.path().join("sender");
    let recv_home = tmp.path().join("recv");
    for h in [&sender_home, &recv_home] {
        std::fs::create_dir_all(h.join(".config")).unwrap();
        std::fs::create_dir_all(h.join("Documents")).unwrap();
    }
    let sender = FakePlatform::with_home(&sender_home);
    let recv = FakePlatform::with_home(&recv_home)
        .with_state_dir(tmp.path().join("state"))
        .binary("app", "/usr/bin/app");
    Env {
        tmp,
        sender_home,
        recv_home,
        sender,
        recv,
    }
}

/// Manifest entry of a sender path, with a fixed mtime (deterministic JSON).
fn entry(e: &Env, root: SemanticRoot, rel: &str) -> Entry {
    let mut en = entry_from_path(&e.sender, root, rel, DEFAULT_CHUNK_PARAMS).unwrap();
    en.mtime_ns = Some(MTIME_NS);
    en
}

fn item(recipe: Option<&str>, id: &str, entries: Vec<Entry>) -> ManifestItem {
    ManifestItem {
        recipe_id: recipe.map(str::to_owned),
        item_id: id.into(),
        label: id.into(),
        flags: ItemFlags::default(),
        entries,
        handler: None,
        artifacts: Vec::new(),
        source_paths: Vec::new(),
    }
}

fn manifest(items: Vec<ManifestItem>) -> Manifest {
    let m = Manifest::new(
        0,
        SourceInfo {
            os: OsKind::Linux,
            carrova_version: "test".into(),
            device_name: None,
        },
        items,
    );
    m.validate().unwrap();
    m
}

fn recipe() -> Recipe {
    carrova_recipe::load_str(
        r"
schema: 2
id: app
name: App
category: test
detect:
  linux:
    - binary: app
processes: [app-daemon]
items:
  - id: config
    label: Config
    paths:
      - { root: user_config, rel: app }
  - id: keyring
    label: Keyring
    handler: gpg-keyring
    paths:
      - { root: home, rel: .gnupg/pubring.kbx }
      - { root: home, rel: .gnupg/trustdb.gpg }
    flags:
      requires_closed: true
",
        Path::new("app/recipe.yaml"),
    )
    .unwrap()
}

fn other_recipe() -> Recipe {
    carrova_recipe::load_str(
        r"
schema: 2
id: other
name: Other
category: test
detect:
  linux:
    - binary: other
items:
  - id: data
    label: Data
    paths:
      - { root: user_data, rel: other }
",
        Path::new("other/recipe.yaml"),
    )
    .unwrap()
}

fn sid() -> SessionId {
    SessionId("20261108-000000-abcdef".into())
}

fn artifact(name: &str, bytes: &[u8]) -> Artifact {
    let fc = chunk_reader(bytes, DEFAULT_CHUNK_PARAMS).unwrap();
    Artifact {
        name: name.into(),
        size: fc.size,
        hash: fc.file_hash,
        chunks: fc.chunks,
    }
}

/// A manifest exercising every planner decision, and a receiver set up so
/// that each one happens.
fn rich(e: &Env) -> Manifest {
    let s = (&e.sender, e.sender_home.as_path());
    mkdir(s.0, s.1, ".config/app", 0o755);
    for (n, c, mode) in [
        ("a", "A", 0o644),
        ("b", "B", 0o644),
        ("c", "C", 0o644),
        ("d", "D", 0o644),
        ("e", "E", 0o644),
        ("w", "W", 0o666),
        ("g", "G", 0o664),
    ] {
        put(
            s.0,
            s.1,
            &format!(".config/app/{n}.txt"),
            c.as_bytes(),
            mode,
        );
    }
    put(s.0, s.1, ".config/app/sub/x.txt", b"X", 0o644);
    put(s.0, s.1, ".config/app/via/y.txt", b"Y", 0o644);
    mkdir(s.0, s.1, ".config/app/dir", 0o700);
    put(s.0, s.1, ".config/app/isdir", b"F", 0o644);
    symlink("a.txt", e.sender_home.join(".config/app/link")).unwrap();
    symlink("/etc/passwd", e.sender_home.join(".config/app/escape")).unwrap();
    symlink("../shared/x", e.sender_home.join(".config/app/through")).unwrap();
    put(s.0, s.1, ".bashrc", b"alias x=y", 0o644);
    put(s.0, s.1, ".local/share/other/o.txt", b"O", 0o644);
    put(s.0, s.1, ".local/share/raw/r.txt", b"R", 0o644);

    let uc = |rel: &str| entry(e, SemanticRoot::UserConfig, rel);
    let mut config = item(
        Some("app"),
        "config",
        [
            "app",
            "app/a.txt",
            "app/b.txt",
            "app/c.txt",
            "app/d.txt",
            "app/e.txt",
            "app/w.txt",
            "app/g.txt",
            "app/sub",
            "app/sub/x.txt",
            "app/via/y.txt",
            "app/dir",
            "app/isdir",
            "app/link",
            "app/escape",
            "app/through",
        ]
        .into_iter()
        .map(uc)
        .collect(),
    );
    config.flags.requires_closed = true;

    let shell = item(
        None,
        "home/.bashrc",
        vec![entry(e, SemanticRoot::Home, ".bashrc")],
    );
    let other = item(
        Some("other"),
        "data",
        vec![entry(e, SemanticRoot::UserData, "other/o.txt")],
    );
    let mut keyring = item(Some("app"), "keyring", Vec::new());
    keyring.handler = Some("gpg-keyring".into());
    keyring.artifacts = vec![artifact("pubring.asc", b"-----BEGIN PGP fake")];
    let mut unknown = item(
        None,
        "user_data/raw",
        vec![entry(e, SemanticRoot::UserData, "raw/r.txt")],
    );
    unknown.handler = Some("rm-rf".into());
    let mut ghost = item(Some("ghost"), "x", Vec::new());
    ghost.flags.sensitive = true;

    // Receiver state.
    let r = (&e.recv, e.recv_home.as_path());
    mkdir(r.0, r.1, ".config/app", 0o777);
    put(r.0, r.1, ".config/app/b.txt", b"B", 0o644);
    put(r.0, r.1, ".config/app/c.txt", b"old", 0o644);
    put(r.0, r.1, ".config/app/d.txt", b"old", 0o644);
    put(
        r.0,
        r.1,
        ".config/app/d.txt.carrova-20261108",
        b"taken",
        0o644,
    );
    put(r.0, r.1, ".config/app/e.txt", b"old", 0o644);
    put(r.0, r.1, ".config/app/sub", b"a file", 0o644);
    symlink("/tmp", e.recv_home.join(".config/app/via")).unwrap();
    mkdir(r.0, r.1, ".config/app/isdir", 0o755);
    put(r.0, r.1, ".config/app/dir", b"a file", 0o644);
    symlink("/etc", e.recv_home.join(".config/shared")).unwrap();

    manifest(vec![config, shell, other, keyring, unknown, ghost])
}

fn rich_options() -> PlanOptions {
    let mut o = PlanOptions {
        items: Some(
            [
                "app/config",
                "home/.bashrc",
                "other/data",
                "app/keyring",
                "user_data/raw",
            ]
            .map(carrova_core::ItemKey::from)
            .to_vec(),
        ),
        confirmed_sensitive: vec!["app/keyring".into(), "user_data/raw".into()],
        ..PlanOptions::default()
    };
    o.path_rules
        .insert("user_config:app/d.txt".into(), ConflictRule::KeepBoth);
    o.path_rules
        .insert("user_config:app/e.txt".into(), ConflictRule::Skip);
    o.path_rules
        .insert("user_config:app/nothing".into(), ConflictRule::Skip);
    o.item_rules
        .insert("app/config".into(), ConflictRule::Replace);
    o.remaps.insert(
        "other/data".into(),
        Remap {
            root: Some(SemanticRoot::Documents),
            rel_prefix: Some("moved".into()),
        },
    );
    o
}

fn rich_plans() -> String {
    let e = env();
    let m = rich(&e);
    let recipes = [recipe(), other_recipe()];
    let busy = e
        .recv
        .clone()
        .process(42, "app-daemon")
        .free_space_bytes(1000);
    let blocked = plan_on_date(&busy, &recipes, &m, &rich_options(), &sid(), DATE).unwrap();
    let overridden = PlanOptions {
        ignore_requires_closed: true,
        ignore_free_space: true,
        keep_group_write: true,
        import_trust: true,
        keep_both_suffix: "bak-{date}".into(),
        ..rich_options()
    };
    let overrides = plan_on_date(&busy, &recipes, &m, &overridden, &sid(), DATE).unwrap();
    let idle = e.recv.clone().free_space_bytes(1 << 40);
    let defaults =
        plan_on_date(&idle, &recipes, &m, &PlanOptions::default(), &sid(), DATE).unwrap();
    let relaxed = PlanOptions {
        ignore_requires_closed: true,
        ignore_free_space: true,
        ..PlanOptions::default()
    };
    let disabled = plan_on_date(&idle, &recipes, &m, &relaxed, &sid(), DATE).unwrap();
    scrub(
        &json(&[blocked, overrides, defaults, disabled]),
        e.tmp.path(),
    )
}

#[test]
fn plan_json_and_texts_are_stable() {
    golden("plans.json", &rich_plans());
}

#[test]
fn plan_json_round_trips_byte_identically() {
    let text = std::fs::read_to_string(golden_path("plans.json")).unwrap();
    let plans: Vec<Plan> = serde_json::from_str(&text).unwrap();
    assert_eq!(json(&plans), text);
}

#[test]
fn plans_written_with_string_blockers_still_read() {
    let old = std::fs::read_to_string(golden_path("plans-string-blockers.json")).unwrap();
    let plans: Vec<Plan> = serde_json::from_str(&old).unwrap();
    let current = std::fs::read_to_string(golden_path("plans.json")).unwrap();
    let mut current: Vec<Plan> = serde_json::from_str(&current).unwrap();
    // Old plans predate `safe_picks`: they read with none (Accept all is a no-op).
    for plan in &mut current {
        plan.safe_picks.clear();
    }
    assert_eq!(json(&plans), json(&current));
}

// ---------------------------------------------------------------- journal

fn every_record() -> Vec<Record> {
    let hash = Blake3Hash::of(b"golden");
    vec![
        Record::Begin {
            session: "20261108-000000-abcdef".into(),
            version: 3,
        },
        Record::CreatedDir { abs: "/h/d".into() },
        Record::CreatedTemp {
            abs: "/h/.f.carrova-tmp".into(),
        },
        Record::CreatedFile {
            abs: "/h/f".into(),
            src: "/s/files/0".into(),
            hash,
            size: 6,
            backup: Some("/s/backup/1".into()),
        },
        Record::CreatedFile {
            abs: "/h/g".into(),
            src: "/s/files/1".into(),
            hash,
            size: 6,
            backup: None,
        },
        Record::CreatedSymlink {
            abs: "/h/l".into(),
            target: "f".into(),
        },
        Record::BackedUp {
            abs: "/h/f".into(),
            backup: "/s/backup/1".into(),
            kind: BackupKind::File,
            mode: Some(0o644),
        },
        Record::BackedUp {
            abs: "/h/d2".into(),
            backup: "/s/backup/2".into(),
            kind: BackupKind::Dir,
            mode: None,
        },
        Record::BackedUp {
            abs: "/h/l2".into(),
            backup: "/s/backup/3".into(),
            kind: BackupKind::Symlink,
            mode: Some(0o777),
        },
        Record::ModeChanged {
            abs: "/h/d".into(),
            old_mode: 0o755,
        },
        Record::HandlerDir {
            abs: "/h/.gnupg".into(),
        },
        Record::HandlerBackup {
            item: "app/keyring".into(),
            files: vec![
                ("/h/.gnupg/a".into(), Some("/s/backup/4".into())),
                ("/h/.gnupg/b".into(), None),
            ],
        },
        Record::HandlerDone {
            item: "app/keyring".into(),
            after: vec![
                (
                    "/h/.gnupg/a".into(),
                    Some(Fingerprint::File { hash, size: 6 }),
                ),
                (
                    "/h/.gnupg/l".into(),
                    Some(Fingerprint::Symlink { target: "a".into() }),
                ),
                ("/h/.gnupg/d".into(), Some(Fingerprint::Dir { tree: hash })),
                ("/h/.gnupg/s".into(), Some(Fingerprint::Special)),
                ("/h/.gnupg/b".into(), None),
            ],
        },
        Record::Committed,
        Record::Undone { seq: 7 },
        Record::RolledBack,
    ]
}

#[test]
fn journal_encoding_is_stable() {
    let tmp = TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    let path = tmp.path().join(JOURNAL_FILE);
    let mut j = Journal::create(&p, &path).unwrap();
    for r in every_record() {
        j.append(&r).unwrap();
    }
    drop(j);
    let bytes = std::fs::read(&path).unwrap();
    let hex: String = bytes
        .chunks(32)
        .map(|line| {
            let mut s: String = line.iter().map(|b| format!("{b:02x}")).collect();
            s.push('\n');
            s
        })
        .collect();
    golden("journal.hex", &hex);
    assert_eq!(journal::read(&p, &path).unwrap(), every_record());
}

// ---------------------------------------------------------------- apply

/// Apply the default plan of [`rich`] (staged by hand from the sender
/// files): events and the journal it writes.
#[test]
fn apply_events_and_journal_are_stable() {
    let e = env();
    let m = rich(&e);
    let recv = e.recv.clone().free_space_bytes(1 << 40);
    let recipes = [recipe(), other_recipe()];
    let p = plan_on_date(&recv, &recipes, &m, &PlanOptions::default(), &sid(), DATE).unwrap();
    assert!(p.blockers.is_empty(), "{:?}", p.blockers);

    let session_dir = e.tmp.path().join("session");
    let files_dir = session_dir.join("staging/files");
    std::fs::create_dir_all(&files_dir).unwrap();
    let mut staged = Staged::default();
    for (n, a) in p.actions.iter().enumerate() {
        let src = m
            .items
            .iter()
            .find(|i| carrova_core::ItemKey::of(i) == a.item)
            .map(|i| &i.entries[a.entry_index])
            .unwrap();
        let from = e
            .sender
            .resolve_root(src.root)
            .unwrap()
            .join(src.rel.as_str());
        if a.kind != carrova_core::ObjectKind::File {
            continue;
        }
        let to = files_dir.join(n.to_string());
        std::fs::copy(&from, &to).unwrap();
        recv.set_mode(&to, 0o600).unwrap();
        staged.files.insert((a.item.clone(), a.entry_index), to);
    }
    let sink = VecSink::default();
    let report = apply(&recv, &p, &m, &staged, &session_dir, &sink, Failpoint::OFF).unwrap();
    let records = journal::read(&recv, &session_dir.join(JOURNAL_FILE)).unwrap();
    let out = format!(
        "{}{}{}",
        json(&sink.events()),
        json(&records),
        json(&(&report.written, &report.backed_up, &report.handlers_run)),
    );
    golden("apply.json", &scrub(&out, e.tmp.path()));

    // And its rollback.
    let sink = VecSink::default();
    let rb = rollback(&recv, &session_dir, &sink).unwrap();
    let out = format!("{}{}", json(&sink.events()), report_text(&rb));
    golden("apply_rollback.txt", &scrub(&out, e.tmp.path()));
}

// ---------------------------------------------------------------- rollback

fn report_text(r: &carrova_core::RollbackReport) -> String {
    let mut out = String::new();
    for t in &r.removed {
        out += &format!("removed {t}\n");
    }
    for t in &r.restored {
        out += &format!("restored {t}\n");
    }
    for q in &r.quarantined {
        out += &format!("quarantined {} -> {}\n", q.target, q.moved_to.display());
    }
    for f in &r.failed {
        out += &format!("failed {}: {}\n", f.target, f.reason);
    }
    out
}

/// A hand-written journal whose undo hits every rollback outcome and
/// failure reason.
#[test]
fn rollback_outcomes_and_reasons_are_stable() {
    let tmp = TempDir::new().unwrap();
    let h = tmp.path().join("h");
    let s = tmp.path().join("session");
    std::fs::create_dir_all(&h).unwrap();
    std::fs::create_dir_all(s.join("backup")).unwrap();
    let p = FakePlatform::with_home(&h);
    let w = |rel: &str, bytes: &[u8]| {
        let path = h.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    };
    let d = |rel: &str| {
        let path = h.join(rel);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let ours = Blake3Hash::of(b"ours");
    let gone = s.join("gone");

    let temp_dir = d("temp_dir");
    let removed = w("removed", b"ours");
    let changed = w("changed", b"theirs");
    let now_dir = d("now_dir");
    let link_ok = h.join("link_ok");
    symlink("t", &link_ok).unwrap();
    let link_moved = h.join("link_moved");
    symlink("other", &link_moved).unwrap();
    let link_file = w("link_file", b"x");
    let empty = d("empty");
    let full = d("full");
    w("full/f", b"x");
    let dir_file = w("dir_file", b"x");
    let mode_dir = d("mode_dir");
    let occupied = w("occupied", b"new");
    let occupied_backup = s.join("backup/1");
    std::fs::write(&occupied_backup, b"old").unwrap();
    let restored = h.join("restored");
    let restored_backup = s.join("backup/2");
    std::fs::write(&restored_backup, b"orig").unwrap();
    let hdir = d("hdir");
    w("hdir/foreign", b"x");
    let hblocked = d("hblocked");
    let hblocked_inner = d("hblocked/inner");
    w("hblocked/inner/f", b"x");
    let hfile = w("hfile", b"changed later");
    let hfile_backup = s.join("backup/3");
    std::fs::write(&hfile_backup, b"before").unwrap();
    let hnew = w("hnew", b"handler output, changed");

    let records = vec![
        Record::Begin {
            session: "x".into(),
            version: 3,
        },
        Record::CreatedTemp { abs: temp_dir },
        Record::CreatedFile {
            abs: removed,
            src: gone.clone(),
            hash: ours,
            size: 4,
            backup: None,
        },
        Record::CreatedFile {
            abs: changed,
            src: gone.clone(),
            hash: ours,
            size: 4,
            backup: None,
        },
        Record::CreatedFile {
            abs: now_dir,
            src: gone.clone(),
            hash: ours,
            size: 4,
            backup: None,
        },
        Record::CreatedSymlink {
            abs: link_ok,
            target: "t".into(),
        },
        Record::CreatedSymlink {
            abs: link_moved,
            target: "t".into(),
        },
        Record::CreatedSymlink {
            abs: link_file,
            target: "t".into(),
        },
        Record::CreatedDir { abs: empty },
        Record::CreatedDir { abs: full },
        Record::CreatedDir { abs: dir_file },
        Record::ModeChanged {
            abs: mode_dir,
            old_mode: 0o700,
        },
        Record::BackedUp {
            abs: occupied,
            backup: occupied_backup,
            kind: BackupKind::File,
            mode: Some(0o644),
        },
        Record::BackedUp {
            abs: restored,
            backup: restored_backup,
            kind: BackupKind::File,
            mode: Some(0o644),
        },
        Record::CreatedDir {
            abs: hblocked_inner,
        },
        Record::HandlerDir { abs: hblocked },
        Record::HandlerDir { abs: hdir },
        Record::HandlerBackup {
            item: "app/keyring".into(),
            files: vec![(hfile.clone(), Some(hfile_backup)), (hnew.clone(), None)],
        },
        Record::HandlerDone {
            item: "app/keyring".into(),
            after: vec![
                (
                    hfile,
                    Some(Fingerprint::File {
                        hash: Blake3Hash::of(b"handler"),
                        size: 7,
                    }),
                ),
                (
                    hnew,
                    Some(Fingerprint::File {
                        hash: Blake3Hash::of(b"handler output"),
                        size: 14,
                    }),
                ),
            ],
        },
    ];
    let mut j = Journal::create(&p, &s.join(JOURNAL_FILE)).unwrap();
    for r in &records {
        j.append(r).unwrap();
    }
    drop(j);

    let sink = VecSink::default();
    let first = rollback(&p, &s, &sink).unwrap();
    let again = rollback(&p, &s, &VecSink::default()).unwrap();
    let out = format!(
        "{}{}--- second run\n{}",
        json(&sink.events()),
        report_text(&first),
        report_text(&again),
    );
    golden("rollback.txt", &scrub(&out, tmp.path()));
}

#[test]
fn rollback_without_journal_and_bad_version_texts_are_stable() {
    let tmp = TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    let none = rollback(&p, tmp.path(), &VecSink::default()).unwrap_err();
    let mut j = Journal::create(&p, &tmp.path().join(JOURNAL_FILE)).unwrap();
    j.append(&Record::Begin {
        session: "x".into(),
        version: 99,
    })
    .unwrap();
    drop(j);
    let bad = rollback(&p, tmp.path(), &VecSink::default()).unwrap_err();
    golden("rollback_errors.txt", &format!("{none}\n{bad}\n"));
}
