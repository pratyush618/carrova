//! Manual picks end to end: a scan-less send session picks files and
//! folders by path, exports them, and a second engine restores them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use carrova_core::plan::Blocker;
use carrova_core::select::{PickRejection, PickResult, SelectionEdit};
use carrova_core::{
    Cancel, Engine, Exporter, Importer, KdfParams, Passphrase, PlanOptions, ScanOptions, Selection,
    VecSink, WriteOptions,
};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{OsKind, Platform};
use tempfile::TempDir;

const RECIPE: &str = r"
schema: 2
id: app
name: The App
category: test
detect:
  linux:
    - binary: app
items:
  - id: config
    label: Config
    paths:
      - { root: user_config, rel: app }
";

/// One machine: home + separate state dir.
struct Side {
    tmp: TempDir,
    home: PathBuf,
    platform: Arc<FakePlatform>,
    engine: Engine,
}

impl Side {
    fn new(os: OsKind) -> Self {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(home.join(".config/app")).unwrap();
        let platform = Arc::new(
            FakePlatform::with_home(&home)
                .with_os(os)
                .with_state_dir(tmp.path().join("state/carrova"))
                .binary("app", "/usr/bin/app"),
        );
        let engine = Engine::new(
            platform.clone(),
            common::recipes(RECIPE),
            Arc::new(VecSink::default()),
        );
        Self {
            tmp,
            home,
            platform,
            engine,
        }
    }

    fn put(&self, rel: &str, bytes: &[u8], mode: u32) {
        common::put(&*self.platform, &self.home, rel, bytes, mode);
    }

    fn manual(&self) -> Exporter {
        Exporter::start_manual(self.engine.clone(), Cancel::default()).unwrap()
    }

    fn paths(&self, rels: &[&str]) -> Vec<PathBuf> {
        rels.iter().map(|r| self.home.join(r)).collect()
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.home.join(rel)).unwrap()
    }
}

fn pass() -> Passphrase {
    Passphrase::new("correct horse battery staple".into())
}

/// Pick `rels` on `sender` (confirming the sensitive ones), select and
/// export; returns the archive path and the exporter.
fn export_picks(sender: &Side, rels: &[&str]) -> (PathBuf, Exporter) {
    let mut exp = sender.manual();
    let mut sel = Selection::default();
    let outcomes = exp.pick(&mut sel, &sender.paths(rels));
    assert!(
        outcomes
            .iter()
            .all(|o| matches!(o.result, PickResult::Added { .. })),
        "{outcomes:?}"
    );
    confirm_sensitive(&mut sel);
    exp.select(sel).unwrap();
    let path = sender.tmp.path().join("out.carrova");
    let floor = WriteOptions {
        kdf: KdfParams::FLOOR,
    };
    exp.export(&path, &pass(), floor).unwrap();
    (path, exp)
}

fn confirm_sensitive(sel: &mut Selection) {
    let sensitive: Vec<_> = sel
        .items
        .iter()
        .filter(|(_, c)| c.sensitive)
        .map(|(k, _)| k.clone())
        .collect();
    for item in sensitive {
        sel.apply_all([
            SelectionEdit::ConfirmSensitive {
                item: item.clone(),
                confirmed: true,
            },
            SelectionEdit::Item {
                item,
                included: true,
            },
        ])
        .unwrap();
    }
}

/// Receiver options importing every item of `archive` (raw items arrive
/// unticked, so each is confirmed).
fn everything(receiver: &Side, archive: &Path) -> PlanOptions {
    let imp = Importer::open(receiver.engine.clone(), archive, &pass()).unwrap();
    PlanOptions {
        confirmed_sensitive: imp
            .manifest()
            .unwrap()
            .items
            .iter()
            .map(|i| i.item_id.as_str().into())
            .collect(),
        ..PlanOptions::default()
    }
}

#[test]
fn manual_picks_round_trip_deep_folder_file_and_key() {
    let sender = Side::new(OsKind::Linux);
    sender.put("proj/deep/er/b.txt", b"deep file", 0o644);
    sender.put("proj/a.txt", b"top file", 0o644);
    sender.put("notes.txt", b"my notes", 0o644);
    sender.put("site/.env", b"TOKEN=secret", 0o600);
    let (archive, _) = export_picks(&sender, &["proj", "notes.txt", "site/.env"]);

    let receiver = Side::new(OsKind::Linux);
    let options = everything(&receiver, &archive);
    let mut imp = Importer::open(receiver.engine.clone(), &archive, &pass()).unwrap();
    imp.plan(&options).unwrap();
    imp.approve().unwrap();
    imp.apply().unwrap();

    for rel in ["proj/deep/er/b.txt", "proj/a.txt", "notes.txt", "site/.env"] {
        assert_eq!(receiver.read(rel), sender.read(rel), "{rel}");
    }
    let flags: Vec<_> = imp
        .manifest()
        .unwrap()
        .items
        .iter()
        .map(|i| (i.item_id.as_str(), i.flags.sensitive))
        .collect();
    assert_eq!(
        flags,
        [
            ("home/notes.txt", false),
            ("home/proj", false),
            ("home/site/.env", true)
        ]
    );
    let env = receiver.platform.metadata(&receiver.home.join("site/.env"));
    if receiver.platform.permission_model() == carrova_platform::PermissionModel::Unix {
        assert_eq!(env.unwrap().mode, Some(0o600));
    }
}

#[test]
fn rejections_are_reported_and_change_nothing() {
    let sender = Side::new(OsKind::Linux);
    let exp = sender.manual();
    let mut sel = Selection::default();

    let outcomes = exp.pick(
        &mut sel,
        &[PathBuf::from("/etc/passwd"), sender.home.clone()],
    );

    let results: Vec<_> = outcomes.into_iter().map(|o| o.result).collect();
    assert_eq!(
        results,
        [
            PickResult::Rejected {
                reason: PickRejection::OutsideRoots
            },
            PickResult::Rejected {
                reason: PickRejection::IsRoot {
                    root: carrova_platform::SemanticRoot::Home
                }
            },
        ]
    );
    assert!(sel.items.is_empty());
}

#[test]
fn pick_key_names_a_path_like_its_pick() {
    let sender = Side::new(OsKind::Linux);
    sender.put(".config/tool/cfg", b"x", 0o644);
    let exp = sender.manual();

    let key = exp.pick_key(&sender.home.join(".config/tool"));

    assert_eq!(key.unwrap().as_str(), "raw:user_config/tool");
}

#[test]
fn manual_session_scans_later() {
    let sender = Side::new(OsKind::Linux);
    let mut exp = sender.manual();
    assert!(exp.scan().apps.is_empty());

    let ids: Vec<_> = exp
        .scan_now(ScanOptions::default())
        .unwrap()
        .apps
        .iter()
        .filter(|a| a.detected)
        .map(|a| a.id.clone())
        .collect();

    assert_eq!(ids, ["app"]);
    assert!(exp.scan().apps.iter().any(|a| a.id == "app"));
}

#[test]
fn export_lists_pick_deleted_after_selection_as_skipped() {
    let sender = Side::new(OsKind::Linux);
    sender.put("notes.txt", b"my notes", 0o644);
    sender.put("proj/a.txt", b"kept", 0o644);
    let mut exp = sender.manual();
    let mut sel = Selection::default();
    let _ = exp.pick(&mut sel, &sender.paths(&["notes.txt", "proj"]));
    exp.select(sel).unwrap();
    std::fs::remove_file(sender.home.join("notes.txt")).unwrap();

    let path = sender.tmp.path().join("out.carrova");
    let floor = WriteOptions {
        kdf: KdfParams::FLOOR,
    };
    let (_, checklist) = exp.export(&path, &pass(), floor).unwrap();

    let skipped: Vec<_> = checklist.skipped.iter().map(|s| s.what.as_str()).collect();
    assert_eq!(skipped, ["raw:home/notes.txt"]);
    assert_eq!(checklist.moved, ["home/proj"]);
}

#[test]
fn picks_from_another_os_are_blocked() {
    let sender = Side::new(OsKind::Linux);
    sender.put("notes.txt", b"my notes", 0o644);
    let (archive, _) = export_picks(&sender, &["notes.txt"]);

    let receiver = Side::new(OsKind::MacOs);
    let options = everything(&receiver, &archive);
    let mut imp = Importer::open(receiver.engine.clone(), &archive, &pass()).unwrap();
    let plan = imp.plan(&options).unwrap();

    assert_eq!(
        plan.blockers,
        [Blocker::CrossOs {
            item: "home/notes.txt".into(),
            from: OsKind::Linux,
            to: OsKind::MacOs,
        }]
    );
}

#[test]
fn secret_appearing_in_a_picked_folder_after_selection_is_held_back() {
    let sender = Side::new(OsKind::Linux);
    sender.put("proj/a.txt", b"kept", 0o644);
    sender.put("site/.env", b"CONFIRMED=1", 0o600);
    let mut exp = sender.manual();
    let mut sel = Selection::default();
    let _ = exp.pick(&mut sel, &sender.paths(&["proj", "site"]));
    confirm_sensitive(&mut sel);
    exp.select(sel).unwrap();
    sender.put("proj/.env", b"TOKEN=late", 0o600);
    sender.put("proj/.ssh/id_ed25519", b"late key", 0o600);

    let path = sender.tmp.path().join("out.carrova");
    let floor = WriteOptions {
        kdf: KdfParams::FLOOR,
    };
    let (_, checklist) = exp.export(&path, &pass(), floor).unwrap();

    let skipped: Vec<_> = checklist
        .skipped
        .iter()
        .map(|s| (s.what.as_str(), s.reason.as_str()))
        .collect();
    let held = "looks like a key or password; pick it again to confirm it";
    assert_eq!(
        skipped,
        [("home:proj/.env", held), ("home:proj/.ssh", held)]
    );
    let imp = Importer::open(sender.engine.clone(), &path, &pass()).unwrap();
    let entries: Vec<_> = imp
        .manifest()
        .unwrap()
        .items
        .iter()
        .flat_map(|i| i.entries.iter().map(|e| e.rel.as_str().to_owned()))
        .collect();
    assert!(entries.contains(&"proj/a.txt".to_owned()), "{entries:?}");
    assert!(entries.contains(&"site/.env".to_owned()), "{entries:?}");
    assert!(
        !entries.iter().any(|e| e.starts_with("proj/.")),
        "{entries:?}"
    );
}
