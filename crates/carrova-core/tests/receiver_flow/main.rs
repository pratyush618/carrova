//! Receiver planning (`plan.rs`) and staging (`stage.rs`) with tempdir homes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use carrova_archive::{ArchiveReader, WriteOptions, write_archive};
use carrova_core::plan::{
    Action, ConflictRule, ObjectKind, Plan, PlanOptions, Remap, plan, plan_on_date,
};
use carrova_core::session::{Event, NullSink, Phase, SessionId, VecSink};
use carrova_core::test_support::stage::{stage, wipe};
use carrova_core::{ArtifactId, Cancel, CoreError, ItemKey};
use carrova_crypto::{KdfParams, Passphrase};
use carrova_manifest::{
    Artifact, Blake3Hash, ChunkRef, ChunkSource, ChunkSourceError, DEFAULT_CHUNK_PARAMS, Entry,
    ItemFlags, Manifest, ManifestItem, SourceInfo, chunk_reader, entry_from_path,
};
use carrova_platform::fake::{FakePlatform, symlink};
use carrova_platform::{Platform, SemanticRoot};
use carrova_recipe::Recipe;
use common::{mkdir, put};
use tempfile::TempDir;

const DATE: &str = "20261108";

// ---------------------------------------------------------------- helpers

struct Env {
    _tmp: TempDir,
    sender_home: PathBuf,
    recv_home: PathBuf,
    sender: FakePlatform,
    recv: FakePlatform,
}

fn env() -> Env {
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
        _tmp: tmp,
        sender_home,
        recv_home,
        sender,
        recv,
    }
}

fn entry(p: &FakePlatform, root: SemanticRoot, rel: &str) -> Entry {
    entry_from_path(p, root, rel, DEFAULT_CHUNK_PARAMS).unwrap()
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
    }
}

fn manifest(items: Vec<ManifestItem>) -> Manifest {
    let m = Manifest::new(
        0,
        SourceInfo {
            os: "linux".into(),
            carrova_version: "test".into(),
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

fn sid() -> SessionId {
    SessionId("20261108-000000-abcdef".into())
}

fn run(e: &Env, m: &Manifest, o: &PlanOptions) -> Plan {
    plan_on_date(&e.recv, &[recipe()], m, o, &sid(), DATE).unwrap()
}

fn action_of<'p>(p: &'p Plan, rel: &str) -> &'p Action {
    &p.actions
        .iter()
        .find(|a| a.rel == rel)
        .unwrap_or_else(|| panic!("no action for {rel}: {:#?}", p.actions))
        .action
}

fn is_skip(a: &Action, needle: &str) -> bool {
    matches!(a, Action::Skip { reason } if reason.contains(needle))
}

/// Full receiver tree: path → (kind, content or link target, mode).
fn snapshot(p: &FakePlatform, dir: &Path) -> BTreeMap<PathBuf, (String, Vec<u8>, Option<u32>)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let path = e.unwrap().path();
            let md = std::fs::symlink_metadata(&path).unwrap();
            let mode = p.metadata(&path).unwrap().mode;
            let v = if md.file_type().is_symlink() {
                let t = std::fs::read_link(&path).unwrap();
                ("link".into(), t.to_string_lossy().as_bytes().to_vec(), None)
            } else if md.is_dir() {
                stack.push(path.clone());
                ("dir".into(), Vec::new(), mode)
            } else {
                ("file".into(), std::fs::read(&path).unwrap(), mode)
            };
            out.insert(path, v);
        }
    }
    out
}

/// Standard sender layout: `.config/app/{a,b,c,d}.txt`.
fn app_manifest(e: &Env) -> Manifest {
    mkdir(&e.sender, &e.sender_home, ".config/app", 0o755);
    for (n, c) in [("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")] {
        put(
            &e.sender,
            &e.sender_home,
            &format!(".config/app/{n}.txt"),
            c.as_bytes(),
            0o644,
        );
    }
    let entries = ["app", "app/a.txt", "app/b.txt", "app/c.txt", "app/d.txt"]
        .iter()
        .map(|r| entry(&e.sender, SemanticRoot::UserConfig, r))
        .collect();
    manifest(vec![item(Some("app"), "config", entries)])
}

// ---------------------------------------------------------------- plan

#[test]
fn item_keys() {
    assert_eq!(
        ItemKey::of(&item(Some("git"), "config", vec![])),
        "git/config"
    );
    assert_eq!(
        ItemKey::of(&item(None, "raw:home/.x", vec![])),
        "raw:home/.x"
    );
}

#[test]
fn create_identical_replace_and_default_selection() {
    let e = env();
    let m = app_manifest(&e);
    mkdir(&e.recv, &e.recv_home, ".config/app", 0o755);
    put(&e.recv, &e.recv_home, ".config/app/b.txt", b"B", 0o644);
    put(&e.recv, &e.recv_home, ".config/app/c.txt", b"old", 0o644);
    put(&e.recv, &e.recv_home, ".config/app/d.txt", b"D", 0o600); // mode differs

    let p = run(&e, &m, &PlanOptions::default());
    assert_eq!(*action_of(&p, "app"), Action::Identical);
    assert_eq!(*action_of(&p, "app/a.txt"), Action::Create);
    assert_eq!(*action_of(&p, "app/b.txt"), Action::Identical);
    assert_eq!(*action_of(&p, "app/c.txt"), Action::Replace);
    assert_eq!(*action_of(&p, "app/d.txt"), Action::Replace);
    let a = p.actions.iter().find(|a| a.rel == "app/a.txt").unwrap();
    assert_eq!(a.abs, e.recv_home.join(".config/app/a.txt"));
    assert_eq!((a.kind, a.size, a.mode), (ObjectKind::File, 1, Some(0o644)));
    assert_eq!(a.link_target, None);
    assert_eq!(a.mtime_ns, m.items[0].entries[1].mtime_ns);
    assert!(a.mtime_ns.is_some());
    assert_eq!(p.conflicts().count(), 2);
    assert_eq!(p.bytes_to_write, 3);
    assert!(p.blockers.is_empty(), "{:?}", p.blockers);
    assert!(p.missing_apps.is_empty());
    assert_eq!(p.session, sid());

    // Deterministic.
    assert_eq!(p, run(&e, &m, &PlanOptions::default()));
}

#[test]
fn keep_both_names_avoid_collisions() {
    let e = env();
    let m = app_manifest(&e);
    put(&e.recv, &e.recv_home, ".config/app/c.txt", b"old", 0o644);
    put(
        &e.recv,
        &e.recv_home,
        ".config/app/c.txt.carrova-20261108",
        b"x",
        0o644,
    );
    put(&e.recv, &e.recv_home, ".config/app/d.txt", b"old", 0o644);
    let o = PlanOptions {
        default_rule: ConflictRule::KeepBoth,
        ..PlanOptions::default()
    };
    let p = run(&e, &m, &o);
    assert_eq!(
        *action_of(&p, "app/c.txt"),
        Action::KeepBoth {
            rel: "app/c.txt.carrova-20261108-2".into()
        }
    );
    assert_eq!(
        *action_of(&p, "app/d.txt"),
        Action::KeepBoth {
            rel: "app/d.txt.carrova-20261108".into()
        }
    );
    // Dir mode conflict + keep-both: kept (dirs never get a second copy).
    assert!(matches!(
        action_of(&p, "app"),
        Action::Identical | Action::Skip { .. }
    ));

    // Custom template; a separator in it is refused.
    let o2 = PlanOptions {
        keep_both_suffix: "from-old-{date}".into(),
        ..o.clone()
    };
    assert_eq!(
        *action_of(&run(&e, &m, &o2), "app/d.txt"),
        Action::KeepBoth {
            rel: "app/d.txt.from-old-20261108".into()
        }
    );
    let bad = PlanOptions {
        keep_both_suffix: "../x".into(),
        ..o
    };
    let err = plan_on_date(&e.recv, &[recipe()], &m, &bad, &sid(), DATE).unwrap_err();
    assert!(matches!(err, CoreError::Selection(_)), "{err}");
}

#[test]
fn item_and_path_rules() {
    let e = env();
    let m = app_manifest(&e);
    put(&e.recv, &e.recv_home, ".config/app/c.txt", b"old", 0o644);
    put(&e.recv, &e.recv_home, ".config/app/d.txt", b"old", 0o644);
    let mut o = PlanOptions::default();
    o.item_rules.insert("app/config".into(), ConflictRule::Skip);
    o.path_rules
        .insert("user_config:app/c.txt".into(), ConflictRule::Replace);
    o.path_rules
        .insert("user_config:app/nope".into(), ConflictRule::Skip);
    let p = run(&e, &m, &o);
    assert_eq!(*action_of(&p, "app/c.txt"), Action::Replace);
    assert_eq!(
        *action_of(&p, "app/d.txt"),
        Action::Skip {
            reason: "kept existing (rule: skip)".into()
        }
    );
    // User-chosen skips are not warnings; an unmatched path rule is.
    assert!(!p.warnings.iter().any(|w| w.message.contains("d.txt")));
    assert!(p.warnings.iter().any(|w| w.message.contains("app/nope")));
    assert_eq!(p.bytes_to_write, 3); // a + b + c
}

#[test]
fn dir_vs_file_conflicts_skip() {
    let e = env();
    let m = app_manifest(&e);
    mkdir(&e.recv, &e.recv_home, ".config/app/a.txt", 0o755); // dir where file comes
    let p = run(&e, &m, &PlanOptions::default());
    assert!(is_skip(
        action_of(&p, "app/a.txt"),
        "directory is never replaced"
    ));
    assert!(p.warnings.iter().any(|w| w.message.contains("app/a.txt")));

    // Existing file where the incoming directory goes: dir and children skip.
    let e = env();
    let m = app_manifest(&e);
    put(&e.recv, &e.recv_home, ".config/app", b"file", 0o644);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(is_skip(action_of(&p, "app"), "kept existing"));
    assert!(is_skip(action_of(&p, "app/b.txt"), "not a directory"));
    assert_eq!(p.bytes_to_write, 0);
}

#[test]
fn remap_root_and_prefix() {
    let e = env();
    let m = app_manifest(&e);
    let mut o = PlanOptions::default();
    o.remaps.insert(
        "app/config".into(),
        Remap {
            root: Some(SemanticRoot::Documents),
            rel_prefix: Some("restored".into()),
        },
    );
    let p = run(&e, &m, &o);
    let a = p
        .actions
        .iter()
        .find(|a| a.rel == "restored/app/a.txt")
        .unwrap();
    assert_eq!(a.root, SemanticRoot::Documents);
    assert_eq!(a.abs, e.recv_home.join("Documents/restored/app/a.txt"));
    assert_eq!(a.action, Action::Create);
    assert!(p.warnings.iter().any(|w| w.message.starts_with("override:")
        && w.item.as_ref().is_some_and(|k| *k == "app/config")));

    for bad in ["../x", "/abs", "a//b"] {
        o.remaps.insert(
            "app/config".into(),
            Remap {
                root: None,
                rel_prefix: Some(bad.into()),
            },
        );
        let err = plan_on_date(&e.recv, &[recipe()], &m, &o, &sid(), DATE).unwrap_err();
        assert!(matches!(err, CoreError::UnsafePath { .. }), "{bad}: {err}");
    }
}

#[test]
fn symlink_ancestor_and_symlink_root_are_never_written_through() {
    let e = env();
    let m = app_manifest(&e);
    let elsewhere = e.recv_home.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    symlink(&elsewhere, e.recv_home.join(".config/app")).unwrap();
    let p = run(&e, &m, &PlanOptions::default());
    // The incoming dir itself meets an existing symlink → skip.
    assert!(is_skip(action_of(&p, "app"), "kept existing"));
    for f in ["app/a.txt", "app/b.txt"] {
        assert!(is_skip(action_of(&p, f), "is a symlink"), "{f}");
    }
    assert_eq!(p.bytes_to_write, 0);

    // Root itself a symlink.
    let e = env();
    let m = app_manifest(&e);
    std::fs::remove_dir(e.recv_home.join(".config")).unwrap();
    std::fs::create_dir_all(e.recv_home.join("dotfiles")).unwrap();
    symlink(e.recv_home.join("dotfiles"), e.recv_home.join(".config")).unwrap();
    let p = run(&e, &m, &PlanOptions::default());
    assert!(
        p.actions
            .iter()
            .all(|a| is_skip(&a.action, "root user_config is a symlink"))
    );
}

#[test]
fn escaping_symlink_entries_skip() {
    let e = env();
    mkdir(&e.sender, &e.sender_home, ".config/app", 0o755);
    put(&e.sender, &e.sender_home, ".config/app/a.txt", b"A", 0o644);
    let base = e.sender_home.join(".config/app");
    symlink("a.txt", base.join("ok")).unwrap();
    symlink("../../.bashrc", base.join("to_home")).unwrap();
    symlink("../../../../../../etc/passwd", base.join("up")).unwrap();
    symlink("/etc/passwd", base.join("abs")).unwrap();
    let entries = [
        "app",
        "app/a.txt",
        "app/ok",
        "app/to_home",
        "app/up",
        "app/abs",
    ]
    .iter()
    .map(|r| entry(&e.sender, SemanticRoot::UserConfig, r))
    .collect();
    let m = manifest(vec![item(Some("app"), "config", entries)]);
    let p = run(&e, &m, &PlanOptions::default());
    assert_eq!(*action_of(&p, "app/ok"), Action::Create);
    let ok = p.actions.iter().find(|a| a.rel == "app/ok").unwrap();
    assert_eq!(ok.link_target.as_deref(), Some("a.txt"));
    assert_eq!(ok.kind, ObjectKind::Symlink);
    assert_eq!(*action_of(&p, "app/to_home"), Action::Create);
    assert!(is_skip(action_of(&p, "app/up"), "points outside"));
    assert!(is_skip(action_of(&p, "app/abs"), "points outside"));

    // An existing identical link is Identical; a different one conflicts.
    std::fs::create_dir_all(e.recv_home.join(".config/app")).unwrap();
    symlink("a.txt", e.recv_home.join(".config/app/ok")).unwrap();
    symlink("x", e.recv_home.join(".config/app/to_home")).unwrap();
    let p = run(&e, &m, &PlanOptions::default());
    assert_eq!(*action_of(&p, "app/ok"), Action::Identical);
    assert_eq!(*action_of(&p, "app/to_home"), Action::Replace);
}

#[test]
fn sensitive_items_need_confirmation_and_unknown_keys_fail() {
    let e = env();
    put(&e.sender, &e.sender_home, ".ssh/id", b"secret", 0o600);
    let mut secret = item(
        Some("ssh"),
        "keys",
        vec![entry(&e.sender, SemanticRoot::Home, ".ssh/id")],
    );
    secret.flags.sensitive = true;
    let m = manifest(vec![secret]);

    // Default: not imported at all.
    let p = run(&e, &m, &PlanOptions::default());
    assert!(p.actions.is_empty() && p.blockers.is_empty());

    // Selected but unconfirmed: blocker.
    let mut o = PlanOptions {
        items: Some(vec!["ssh/keys".into()]),
        ..PlanOptions::default()
    };
    let p = run(&e, &m, &o);
    assert_eq!(p.actions.len(), 1);
    assert!(
        p.blockers
            .iter()
            .any(|b| b.to_string().contains("sensitive"))
    );

    // Confirmed: fine (also with items = None).
    o.confirmed_sensitive = vec!["ssh/keys".into()];
    assert!(run(&e, &m, &o).blockers.is_empty());
    o.items = None;
    let p = run(&e, &m, &o);
    assert_eq!(p.actions.len(), 1);
    assert!(p.blockers.is_empty());

    for o in [
        PlanOptions {
            items: Some(vec!["nope/x".into()]),
            ..PlanOptions::default()
        },
        PlanOptions {
            confirmed_sensitive: vec!["nope/x".into()],
            ..PlanOptions::default()
        },
    ] {
        let err = plan_on_date(&e.recv, &[], &m, &o, &sid(), DATE).unwrap_err();
        assert!(matches!(err, CoreError::Selection(_)), "{err}");
    }
}

#[test]
fn running_app_blocks_unless_overridden_and_missing_apps_reported() {
    let e = env();
    put(&e.sender, &e.sender_home, ".config/app/a.txt", b"A", 0o644);
    let mut it = item(
        Some("app"),
        "config",
        vec![entry(&e.sender, SemanticRoot::UserConfig, "app/a.txt")],
    );
    it.flags.requires_closed = true;
    let m = manifest(vec![it]);
    let recv = e.recv.clone().process(42, "app-daemon");

    let p = plan_on_date(
        &recv,
        &[recipe()],
        &m,
        &PlanOptions::default(),
        &sid(),
        DATE,
    )
    .unwrap();
    assert_eq!(p.running_apps, vec!["app".to_owned()]);
    assert!(
        p.blockers
            .iter()
            .any(|b| b.to_string().contains("App (`app`) is running"))
    );
    assert_eq!(p.app_name("app"), "App", "named by the local recipe");

    let o = PlanOptions {
        ignore_requires_closed: true,
        ..PlanOptions::default()
    };
    let p = plan_on_date(&recv, &[recipe()], &m, &o, &sid(), DATE).unwrap();
    assert!(p.blockers.is_empty());
    assert!(
        p.warnings
            .iter()
            .any(|w| w.message.contains("override: App is running"))
    );

    // Not running → no blocker.
    assert!(run(&e, &m, &PlanOptions::default()).blockers.is_empty());

    // App not installed / recipe unknown here.
    let bare = FakePlatform::with_home(&e.recv_home);
    let p = plan_on_date(
        &bare,
        &[recipe()],
        &m,
        &PlanOptions::default(),
        &sid(),
        DATE,
    )
    .unwrap();
    assert_eq!(p.missing_apps, vec!["app".to_owned()]);
    // Recipe unknown here: unrecognised data, imported only when confirmed.
    let p = plan_on_date(&e.recv, &[], &m, &PlanOptions::default(), &sid(), DATE).unwrap();
    assert!(p.actions.is_empty() && p.sensitive.contains_key("app/config"));
    let o = PlanOptions {
        confirmed_sensitive: vec!["app/config".into()],
        ..PlanOptions::default()
    };
    let p = plan_on_date(&e.recv, &[], &m, &o, &sid(), DATE).unwrap();
    assert_eq!(p.missing_apps, vec!["app".to_owned()]);
}

#[test]
fn free_space_blocks_unless_overridden() {
    let e = env();
    let m = app_manifest(&e);
    let recv = e.recv.clone().free_space_bytes(1000);
    let p = plan_on_date(
        &recv,
        &[recipe()],
        &m,
        &PlanOptions::default(),
        &sid(),
        DATE,
    )
    .unwrap();
    assert_eq!(p.free_space, Some(1000));
    assert!(
        p.blockers
            .iter()
            .any(|b| b.to_string().contains("free space"))
    );
    let o = PlanOptions {
        ignore_free_space: true,
        ..PlanOptions::default()
    };
    let p = plan_on_date(&recv, &[recipe()], &m, &o, &sid(), DATE).unwrap();
    assert!(p.blockers.is_empty());
    assert!(p.warnings.iter().any(|w| w.message.contains("free space")));
    assert_eq!(carrova_core::plan::required_space(0), 64 * 1024 * 1024);
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

const PUBRING: &[u8] = b"-----BEGIN PGP PUBLIC KEY BLOCK----- fake";

fn handler_item(name: &str) -> ManifestItem {
    let mut it = item(Some("app"), "keyring", Vec::new());
    it.handler = Some(name.into());
    it.artifacts = vec![artifact("pubring.asc", PUBRING)];
    it
}

#[test]
fn handler_items_plan_backups() {
    let e = env();
    let m = manifest(vec![handler_item("gpg-keyring")]);
    // Receiver-side sensitive (handler changes ~/.gnupg): not run by default.
    let p = run(&e, &m, &PlanOptions::default());
    assert!(p.handlers.is_empty());
    assert!(p.sensitive["app/keyring"].contains(".gnupg"));
    let confirmed = PlanOptions {
        confirmed_sensitive: vec!["app/keyring".into()],
        ..PlanOptions::default()
    };
    let p = run(&e, &m, &confirmed);
    assert_eq!(p.handlers.len(), 1);
    let h = &p.handlers[0];
    assert_eq!(
        (h.item.as_str(), h.handler.as_str()),
        ("app/keyring", "gpg-keyring")
    );
    assert_eq!(
        h.backup,
        vec![
            (SemanticRoot::Home, ".gnupg/pubring.kbx".to_owned()),
            (SemanticRoot::Home, ".gnupg/trustdb.gpg".to_owned()),
        ]
    );
    assert_eq!(p.bytes_to_write, PUBRING.len() as u64);

    // Unknown handler: not run, warned.
    let m = manifest(vec![handler_item("nope")]);
    let p = run(&e, &m, &confirmed);
    assert!(p.handlers.is_empty());
    assert!(
        p.warnings
            .iter()
            .any(|w| w.message.contains("unknown handler"))
    );
    // No local recipe item → cannot back up → not run, warned.
    let m = manifest(vec![handler_item("gpg-keyring")]);
    let p = plan_on_date(&e.recv, &[], &m, &confirmed, &sid(), DATE).unwrap();
    assert!(p.handlers.is_empty());
    assert!(
        p.warnings
            .iter()
            .any(|w| w.message.contains("cannot be backed up"))
    );
}

#[test]
fn plan_writes_nothing() {
    let e = env();
    let m = app_manifest(&e);
    put(&e.recv, &e.recv_home, ".config/app/c.txt", b"old", 0o644);
    put(&e.recv, &e.recv_home, ".config/app/d.txt", b"old", 0o644);
    let before = snapshot(&e.recv, &e.recv_home);
    for rule in [
        ConflictRule::Replace,
        ConflictRule::KeepBoth,
        ConflictRule::Skip,
    ] {
        let mut o = PlanOptions {
            default_rule: rule,
            ..PlanOptions::default()
        };
        o.remaps.insert(
            "app/config".into(),
            Remap {
                root: Some(SemanticRoot::Documents),
                rel_prefix: Some("new/deep".into()),
            },
        );
        run(&e, &m, &o);
        run(
            &e,
            &m,
            &PlanOptions {
                default_rule: rule,
                ..PlanOptions::default()
            },
        );
    }
    // Default entry point (today's date) as well.
    plan(&e.recv, &[recipe()], &m, &PlanOptions::default(), &sid()).unwrap();
    assert_eq!(before, snapshot(&e.recv, &e.recv_home));
}

// ---------------------------------------------------------------- stage

/// Chunk bytes by hash, cut from the sender's files and artifact blobs.
#[derive(Default)]
struct MapSource(HashMap<Blake3Hash, Vec<u8>>);

impl MapSource {
    fn add(&mut self, data: &[u8], chunks: &[ChunkRef]) {
        for c in chunks {
            let start = usize::try_from(c.offset).unwrap();
            self.0
                .insert(c.hash, data[start..start + c.len as usize].to_vec());
        }
    }
}

impl ChunkSource for MapSource {
    fn read_chunk(&mut self, chunk: &ChunkRef) -> Result<Vec<u8>, ChunkSourceError> {
        Ok(self.0[&chunk.hash].clone())
    }
}

fn pass() -> Passphrase {
    Passphrase::new("correct horse battery staple".into())
}

/// Write `m` as an archive (writer-floor KDF) next to the homes.
fn archive(e: &Env, m: &Manifest, extra: &[&[u8]]) -> PathBuf {
    let mut src = MapSource::default();
    for en in m.entries() {
        if !en.chunks().is_empty() {
            let mut abs = e.sender.resolve_root(en.root).unwrap();
            abs.extend(en.rel.components());
            src.add(&std::fs::read(abs).unwrap(), en.chunks());
        }
    }
    for it in &m.items {
        for (a, bytes) in it.artifacts.iter().zip(extra) {
            src.add(bytes, &a.chunks);
        }
    }
    let path = e.sender_home.parent().unwrap().join("x.carrova");
    write_archive(
        &e.sender,
        &path,
        &pass(),
        m,
        &mut src,
        WriteOptions {
            kdf: KdfParams::FLOOR,
        },
    )
    .unwrap();
    path
}

fn session_dir(e: &Env) -> PathBuf {
    let d = e.recv.state_dir().unwrap().join("sessions").join(sid().0);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn stage_round_trip_only_needed_data() {
    let e = env();
    // A larger file so it spans several chunks.
    let big: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
    let mut m = app_manifest(&e);
    put(
        &e.sender,
        &e.sender_home,
        ".config/app/big.bin",
        &big,
        0o600,
    );
    m.items[0]
        .entries
        .push(entry(&e.sender, SemanticRoot::UserConfig, "app/big.bin"));
    m.items.push(handler_item("gpg-keyring"));
    m.validate().unwrap();
    let path = archive(&e, &m, &[PUBRING]);

    put(&e.recv, &e.recv_home, ".config/app/b.txt", b"B", 0o644); // identical
    put(&e.recv, &e.recv_home, ".config/app/c.txt", b"old", 0o644); // replace
    put(&e.recv, &e.recv_home, ".config/app/d.txt", b"old", 0o644); // skip by path rule
    // The handler changes files under ~/.gnupg: sensitive, needs confirming.
    let mut o = PlanOptions {
        confirmed_sensitive: vec!["app/keyring".into()],
        ..PlanOptions::default()
    };
    o.path_rules
        .insert("user_config:app/d.txt".into(), ConflictRule::Skip);
    let p = run(&e, &m, &o);
    assert!(p.blockers.is_empty(), "{:?}", p.blockers);

    let sdir = session_dir(&e);
    let staging = sdir.join("staging");
    let sink = VecSink::default();
    let mut reader = ArchiveReader::open(&e.recv, &path, &pass()).unwrap();
    let staged = stage(
        &e.recv,
        &mut reader,
        &p,
        &staging,
        &sink,
        &Cancel::default(),
    )
    .unwrap();

    let idx = |rel: &str| p.actions.iter().find(|a| a.rel == rel).unwrap().entry_index;
    let key = |rel: &str| (ItemKey::from("app/config"), idx(rel));
    assert_eq!(staged.files.len(), 3, "{:?}", staged.files); // a, c, big
    for (rel, want) in [
        ("app/a.txt", &b"A"[..]),
        ("app/c.txt", b"C"),
        ("app/big.bin", &big),
    ] {
        let f = &staged.files[&key(rel)];
        assert_eq!(std::fs::read(f).unwrap(), want, "{rel}");
        assert_eq!(e.recv.metadata(f).unwrap().mode, Some(0o600));
        assert!(f.starts_with(&staging));
    }
    assert!(!staged.files.contains_key(&key("app/b.txt")));
    assert!(!staged.files.contains_key(&key("app/d.txt")));
    let art = &staged.artifacts[&ArtifactId {
        item: "app/keyring".into(),
        name: "pubring.asc".into(),
    }];
    assert_eq!(std::fs::read(art).unwrap(), PUBRING);
    assert!(!staging.join("chunks").exists());
    assert_eq!(e.recv.metadata(&staging).unwrap().mode, Some(0o700));
    let ev = sink.events();
    assert!(ev.iter().any(|e| matches!(
        e,
        Event::Progress {
            phase: Phase::Stage,
            ..
        }
    )));
    match ev.last().unwrap() {
        Event::Progress { done, total, .. } => assert_eq!(done, total),
        other => panic!("{other:?}"),
    }

    // A plan that does not match the archive → Verification, staging wiped.
    let mut bogus = p.clone();
    bogus.actions[1].entry_index = 99;
    let mut reader = ArchiveReader::open(&e.recv, &path, &pass()).unwrap();
    let err = stage(
        &e.recv,
        &mut reader,
        &bogus,
        &staging,
        &NullSink,
        &Cancel::default(),
    )
    .unwrap_err();
    assert!(matches!(err, CoreError::Verification(_)), "{err}");
    assert!(!staging.exists());

    wipe(&e.recv, &staging, &sdir).unwrap(); // missing = ok
}

#[test]
fn stage_hash_mismatch_fails_and_wipes() {
    let e = env();
    let mut m = app_manifest(&e);
    // Chunks are genuine, but the whole-file hash recorded for a.txt is not.
    if let carrova_manifest::EntryKind::File { hash, .. } = &mut m.items[0].entries[1].kind {
        *hash = Blake3Hash::of(b"something else");
    }
    let path = archive(&e, &m, &[]);
    let p = run(&e, &m, &PlanOptions::default());
    let sdir = session_dir(&e);
    let staging = sdir.join("staging");
    let mut reader = ArchiveReader::open(&e.recv, &path, &pass()).unwrap();
    let err = stage(
        &e.recv,
        &mut reader,
        &p,
        &staging,
        &NullSink,
        &Cancel::default(),
    )
    .unwrap_err();
    assert!(
        matches!(&err, CoreError::Verification(l) if l.contains("app/config#1")),
        "{err}"
    );
    assert!(!staging.exists());
    assert!(sdir.exists());
}
