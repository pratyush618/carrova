//! Session engine end to end: two engine instances in one process (sender
//! home A + state dir, receiver home B + its own state dir), real archive.
//!
//! The archive is built once per test binary (floor KDF) and tests are
//! serialised by a lock so at most one Argon2 derivation (64 MiB) runs at a
//! time.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/apply.rs"]
mod apply_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use apply_support::{Snapshot, snapshot};
use carrova_core::store::{LOG_FILE, PLAN_FILE, SELECTION_FILE, STAGING_DIR, Store};
use carrova_core::{
    Checklist, ConflictRule, CoreError, Engine, Event, Exporter, Importer, KdfParams, Passphrase,
    PlanOptions, Preset, Role, ScanOptions, SessionId, State, VecSink, WriteOptions, sessions,
};
#[cfg(unix)]
use carrova_platform::Platform;
use carrova_platform::fake::FakePlatform;
use tempfile::TempDir;

const PASS: &str = "correct horse battery staple";

/// The test app: config (default, app must be closed) and SSH keys
/// (sensitive).
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
    flags:
      default: true
      requires_closed: true
  - id: keys
    label: Keys
    paths:
      - { root: home, rel: .ssh }
    flags:
      sensitive: true
      default: false
";

fn pass() -> Passphrase {
    Passphrase::new(PASS.into())
}

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One machine: home + separate state dir.
struct Side {
    _tmp: TempDir,
    home: PathBuf,
    platform: Arc<FakePlatform>,
    sink: Arc<VecSink>,
    engine: Engine,
}

impl Side {
    fn new(f: impl FnOnce(FakePlatform) -> FakePlatform) -> Self {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(home.join(".config")).unwrap();
        std::fs::create_dir_all(home.join("Documents")).unwrap();
        let platform = Arc::new(f(FakePlatform::with_home(&home)
            .with_state_dir(tmp.path().join("state/carrova"))
            .binary("app", "/usr/bin/app")));
        let sink = Arc::new(VecSink::default());
        let engine = Engine::new(platform.clone(), common::recipes(RECIPE), sink.clone());
        Self {
            _tmp: tmp,
            home,
            platform,
            sink,
            engine,
        }
    }

    fn put(&self, rel: &str, bytes: &[u8], mode: u32) {
        common::put(&*self.platform, &self.home, rel, bytes, mode);
    }

    fn mkdir(&self, rel: &str, mode: u32) {
        common::mkdir(&*self.platform, &self.home, rel, mode);
    }

    #[cfg(unix)]
    fn mode(&self, rel: &str) -> u32 {
        self.platform
            .metadata(&self.home.join(rel))
            .unwrap()
            .mode
            .unwrap()
            & 0o7777
    }

    fn read(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.home.join(rel)).unwrap()
    }

    fn snap(&self) -> Snapshot {
        snapshot(&*self.platform, &self.home)
    }

    fn session_dir(&self, id: &SessionId) -> PathBuf {
        Store::open(&*self.platform)
            .unwrap()
            .session_dir(id)
            .unwrap()
    }

    fn states(&self) -> Vec<State> {
        self.sink
            .events()
            .into_iter()
            .filter_map(|e| match e {
                Event::StateChanged { to, .. } => Some(to),
                _ => None,
            })
            .collect()
    }
}

/// Sender layout.
fn sender() -> Side {
    let s = Side::new(|p| p);
    s.mkdir(".config/app", 0o700);
    s.put(".config/app/settings.json", b"{\"theme\":\"dark\"}", 0o644);
    s.put(".config/app/notes.txt", b"sender notes", 0o640);
    s.mkdir(".config/app/sub", 0o755);
    s.put(".config/app/sub/x.txt", b"sender x", 0o644);
    s.mkdir(".ssh", 0o700);
    s.put(".ssh/id_ed25519", b"PRIVATE KEY BYTES", 0o600);
    s.put(".ssh/config", b"Host *\n", 0o644);
    s
}

struct Shared {
    _sender: Side,
    archive: PathBuf,
    checklist: Checklist,
}

/// Export once per test binary (the only archive build).
fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| {
        let s = sender();
        let mut exp = Exporter::start(
            s.engine.clone(),
            ScanOptions {
                include_undetected: false,
                unrecognised: false,
            },
        )
        .unwrap();
        assert_eq!(exp.state(), State::Scanned);
        let mut sel = exp.suggest(Preset::Suggested);
        assert!(sel.items["app/config"].included);
        assert!(!sel.items["app/keys"].included, "sensitive starts unticked");

        // Sender side: a sensitive item needs explicit confirmation.
        sel.set_included("app/keys", true).unwrap();
        let err = exp.select(sel.clone()).unwrap_err();
        assert!(matches!(err, CoreError::Selection(_)), "{err}");
        assert_eq!(exp.state(), State::Scanned);
        sel.confirm_sensitive("app/keys").unwrap();
        exp.select(sel).unwrap();
        assert_eq!(exp.state(), State::Selected);
        let dir = s.session_dir(exp.id());
        assert!(dir.join(SELECTION_FILE).exists());

        let archive = s.home.join("../out.carrova");
        let (summary, checklist) = exp
            .export(
                &archive,
                &pass(),
                WriteOptions {
                    kdf: KdfParams::FLOOR,
                },
            )
            .unwrap();
        assert!(summary.chunks > 0);
        assert_eq!(exp.state(), State::Done);
        assert_eq!(
            s.states(),
            [State::Selected, State::Transferring, State::Done]
        );
        let rec = sessions::show(&s.engine, exp.id()).unwrap();
        assert_eq!(rec.role, Role::Export);
        assert_eq!(rec.state, State::Done);
        assert_eq!(rec.checklist.as_ref(), Some(&checklist));
        assert!(
            s.sink
                .events()
                .iter()
                .any(|e| matches!(e, Event::Finished { .. }))
        );
        Shared {
            _sender: s,
            archive,
            checklist,
        }
    })
}

fn confirmed_keys() -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    }
}

// ------------------------------------------------------------------ tests

#[test]
fn round_trip_rollback_and_redacted_log() {
    let _g = lock();
    let sh = shared();
    assert_eq!(sh.checklist.moved, ["app/config", "app/keys"]);
    assert!(
        sh.checklist.skipped.is_empty(),
        "{:?}",
        sh.checklist.skipped
    );

    let r = Side::new(|p| p);
    let before = r.snap();
    let mut imp = Importer::open(r.engine.clone(), &sh.archive, &pass()).unwrap();
    assert_eq!(imp.state(), State::Selected);
    assert_eq!(imp.manifest().unwrap().items.len(), 2);

    // Without confirmation the sensitive item is not imported.
    let plan = imp.plan(&PlanOptions::default()).unwrap();
    assert!(plan.actions.iter().all(|a| a.item == "app/config"));
    // Re-plan with the receiver-side confirmation.
    imp.plan(&confirmed_keys()).unwrap();
    assert_eq!(imp.state(), State::Planned);
    assert!(matches!(imp.apply(), Err(CoreError::NotApproved)));
    imp.approve().unwrap();
    let checklist = imp.apply().unwrap();
    assert_eq!(imp.state(), State::Done);
    assert_eq!(
        r.states(),
        [
            State::Planned,
            State::Transferring,
            State::Staged,
            State::Applying,
            State::Done
        ]
    );
    assert_eq!(checklist.moved, ["app/config", "app/keys"]);
    assert!(checklist.skipped.is_empty());
    assert!(checklist.apps_to_install.is_empty());
    assert!(checklist.rollback_available);

    // Bytes + exact modes identical to the sender.
    assert_eq!(r.read(".config/app/settings.json"), b"{\"theme\":\"dark\"}");
    #[cfg(unix)]
    assert_eq!(r.mode(".config/app/settings.json"), 0o644);
    #[cfg(unix)]
    assert_eq!(r.mode(".config/app/notes.txt"), 0o640);
    #[cfg(unix)]
    assert_eq!(r.mode(".config/app"), 0o700);
    #[cfg(unix)]
    assert_eq!(r.mode(".config/app/sub"), 0o755);
    assert_eq!(r.read(".ssh/id_ed25519"), b"PRIVATE KEY BYTES");
    #[cfg(unix)]
    assert_eq!(r.mode(".ssh/id_ed25519"), 0o600);
    #[cfg(unix)]
    assert_eq!(r.mode(".ssh"), 0o700);

    let id = imp.id().clone();
    let dir = r.session_dir(&id);
    assert!(!dir.join(STAGING_DIR).exists(), "staging wiped");
    assert!(dir.join(PLAN_FILE).exists());
    #[cfg(unix)]
    assert_eq!(
        r.platform.metadata(&dir).unwrap().mode.unwrap() & 0o777,
        0o700
    );
    let rec = sessions::show(&r.engine, &id).unwrap();
    assert_eq!(rec.state, State::Done);
    assert!(rec.approved);
    assert_eq!(rec.checklist, Some(checklist));

    // log.jsonl: 600, one JSON object per event, mentions the key path.
    let log = dir.join(LOG_FILE);
    #[cfg(unix)]
    assert_eq!(
        r.platform.metadata(&log).unwrap().mode.unwrap() & 0o777,
        0o600
    );
    let raw = std::fs::read_to_string(&log).unwrap();
    assert!(raw.lines().count() >= 6);
    for line in raw.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("event").is_some() && v.get("ts").is_some());
    }
    assert!(raw.contains("home:.ssh/id_ed25519"));
    assert!(!raw.contains(PASS));

    // Redacted export.
    let out = r.home.join("../log-export.jsonl");
    sessions::export_log(&r.engine, &id, &out).unwrap();
    let red = std::fs::read_to_string(&out).unwrap();
    assert_eq!(red.lines().count(), raw.lines().count());
    assert!(!red.contains(".ssh"), "{red}");
    assert!(red.contains("<redacted>"));
    assert!(red.contains("user_config:app/settings.json"));
    assert!(!red.contains(&*r.home.display().to_string()), "{red}");
    assert!(
        sessions::export_log(&r.engine, &id, &out).is_err(),
        "never overwrites"
    );

    // Manual rollback of the latest Done import restores the tree.
    let report = sessions::rollback(&r.engine, None).unwrap();
    assert!(report.failed.is_empty());
    assert_eq!(r.snap(), before);
    assert_eq!(
        sessions::show(&r.engine, &id).unwrap().state,
        State::RolledBack
    );
    assert!(sessions::rollback(&r.engine, Some(&id)).is_err());

    // Dismiss deletes the rollback point.
    sessions::dismiss(&r.engine, &id).unwrap();
    assert!(!dir.join("journal.cbor").exists());
    assert!(!dir.join("backup").exists());
}

#[test]
fn conflict_rules_replace_keep_both_skip() {
    let _g = lock();
    let sh = shared();
    let r = Side::new(|p| p);
    r.mkdir(".config/app", 0o750);
    r.put(".config/app/settings.json", b"{\"theme\":\"light\"}", 0o600);
    r.put(".config/app/notes.txt", b"receiver notes", 0o644);
    r.mkdir(".config/app/sub", 0o755);
    r.put(".config/app/sub/x.txt", b"receiver x", 0o644);

    let mut imp = Importer::open(r.engine.clone(), &sh.archive, &pass()).unwrap();
    let mut opts = PlanOptions::default();
    opts.path_rules
        .insert("user_config:app/sub/x.txt".into(), ConflictRule::KeepBoth);
    opts.path_rules
        .insert("user_config:app/notes.txt".into(), ConflictRule::Skip);
    imp.plan(&opts).unwrap();
    let conflicts: Vec<String> = r
        .sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::ConflictFound { target, .. } => Some(target),
            _ => None,
        })
        .collect();
    for t in [
        "user_config:app/settings.json",
        "user_config:app/notes.txt",
        "user_config:app/sub/x.txt",
    ] {
        assert!(conflicts.iter().any(|c| c == t), "{t} in {conflicts:?}");
    }
    imp.approve().unwrap();
    let checklist = imp.apply().unwrap();

    // Replace (suggested default).
    assert_eq!(r.read(".config/app/settings.json"), b"{\"theme\":\"dark\"}");
    #[cfg(unix)]
    assert_eq!(r.mode(".config/app/settings.json"), 0o644);
    // Skip.
    assert_eq!(r.read(".config/app/notes.txt"), b"receiver notes");
    // Keep both.
    assert_eq!(r.read(".config/app/sub/x.txt"), b"receiver x");
    let kept: Vec<_> = std::fs::read_dir(r.home.join(".config/app/sub"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("x.txt.carrova-"))
        .collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert_eq!(r.read(&format!(".config/app/sub/{}", kept[0])), b"sender x");
    // Not imported: sensitive, unconfirmed.
    assert!(!r.home.join(".ssh").exists());

    assert_eq!(checklist.moved, ["app/config"]);
    assert!(
        checklist.skipped.iter().any(|s| s.what == "app/config"),
        "{checklist:?}"
    );
    assert!(
        checklist
            .skipped
            .iter()
            .any(|s| s.what == "app/keys" && s.reason.contains("sensitive")),
        "{checklist:?}"
    );
}

#[test]
fn blockers_refuse_approval_until_resolved() {
    let _g = lock();
    let sh = shared();
    let r = Side::new(|p| p.process(42, "app"));
    let before = r.snap();
    let mut imp = Importer::open(r.engine.clone(), &sh.archive, &pass()).unwrap();

    // Sensitive item selected but not confirmed on the receiver.
    let plan = imp
        .plan(&PlanOptions {
            items: Some(vec!["app/keys".into()]),
            ..PlanOptions::default()
        })
        .unwrap();
    assert!(!plan.blockers.is_empty());
    assert!(matches!(imp.approve(), Err(CoreError::PlanBlocked(_))));
    assert!(matches!(imp.apply(), Err(CoreError::NotApproved)));

    // Running app blocks the requires-closed item…
    let plan = imp.plan(&PlanOptions::default()).unwrap();
    assert!(
        plan.blockers.iter().any(|b| b.to_string().contains("app")),
        "{plan:?}"
    );
    assert!(matches!(imp.approve(), Err(CoreError::PlanBlocked(_))));
    // …until the user overrides it (surfaced as a plan warning, carried by
    // the plan only — not re-emitted as an event, so it is shown once).
    let n_warn = |r: &Side| {
        r.sink
            .events()
            .iter()
            .filter(|e| matches!(e, Event::Warning { .. }))
            .count()
    };
    let w0 = n_warn(&r);
    let plan = imp
        .plan(&PlanOptions {
            ignore_requires_closed: true,
            ..PlanOptions::default()
        })
        .unwrap();
    assert!(
        plan.warnings
            .iter()
            .any(|w| w.message.starts_with("override:")),
        "{plan:?}"
    );
    assert_eq!(
        n_warn(&r),
        w0,
        "plan warnings must not be duplicated as events"
    );
    imp.approve().unwrap();
    // A new plan needs a new approval.
    imp.plan(&PlanOptions {
        ignore_requires_closed: true,
        ..PlanOptions::default()
    })
    .unwrap();
    assert!(!imp.record().approved);
    assert!(matches!(imp.apply(), Err(CoreError::NotApproved)));
    assert_eq!(r.snap(), before, "nothing written before approval");
}

#[cfg(feature = "failpoints")]
#[test]
fn failure_mid_apply_rolls_back_byte_identical() {
    let _g = lock();
    let sh = shared();
    let r = Side::new(|p| p);
    r.mkdir(".config/app", 0o750);
    r.put(".config/app/settings.json", b"{\"theme\":\"light\"}", 0o600);
    r.mkdir(".ssh", 0o700);
    r.put(".ssh/id_ed25519", b"OLD KEY", 0o600);
    let before = r.snap();

    let mut imp = Importer::open(r.engine.clone(), &sh.archive, &pass()).unwrap();
    imp.plan(&confirmed_keys()).unwrap();
    imp.approve().unwrap();
    imp.set_failpoint(carrova_core::Failpoint::after(3));
    let err = imp.apply().unwrap_err();
    assert!(matches!(err, CoreError::RolledBack { .. }), "{err}");
    assert_eq!(imp.state(), State::RolledBack);
    assert_eq!(r.snap(), before);
    let dir = r.session_dir(imp.id());
    assert!(!dir.join(STAGING_DIR).exists());
    let rec = sessions::show(&r.engine, imp.id()).unwrap();
    assert_eq!(rec.state, State::RolledBack);
    assert!(rec.error.unwrap().contains("rolled back"));
    assert!(
        r.sink
            .events()
            .iter()
            .any(|e| matches!(e, Event::RollbackStep { .. }))
    );
}

#[test]
fn resume_from_planned_keeps_or_resets_approval() {
    let _g = lock();
    let sh = shared();
    let r = Side::new(|p| p);
    let id = {
        let mut imp = Importer::open(r.engine.clone(), &sh.archive, &pass()).unwrap();
        imp.plan(&confirmed_keys()).unwrap();
        imp.approve().unwrap();
        imp.id().clone()
    }; // "restart"

    // Wrong passphrase: refused, session untouched.
    assert!(
        Importer::resume_archive(r.engine.clone(), &id, &Passphrase::new("nope".into())).is_err()
    );

    let imp = Importer::resume_archive(r.engine.clone(), &id, &pass()).unwrap();
    assert_eq!(imp.state(), State::Planned);
    assert!(imp.record().approved, "unchanged plan keeps its approval");
    drop(imp);

    // Interrupted while staging (Transferring, stale staging dir) and the
    // receiver changed meanwhile: restage from scratch, plan differs →
    // approve again.
    let store = Store::open(&*r.platform).unwrap();
    let mut rec = store.load(&*r.platform, &id).unwrap();
    rec.state = State::Transferring;
    store.save(&*r.platform, &rec).unwrap();
    let staging = r.session_dir(&id).join(STAGING_DIR);
    std::fs::create_dir_all(staging.join("chunks")).unwrap();
    r.mkdir(".config/app", 0o700);
    r.put(".config/app/settings.json", b"new local file", 0o644);
    let mut imp = Importer::resume_archive(r.engine.clone(), &id, &pass()).unwrap();
    assert_eq!(imp.state(), State::Planned);
    assert!(!staging.exists(), "stale staging wiped");
    assert!(!imp.record().approved);
    assert!(matches!(imp.apply(), Err(CoreError::NotApproved)));
    imp.approve().unwrap();
    imp.apply().unwrap();
    assert_eq!(imp.state(), State::Done);
    assert_eq!(r.read(".config/app/settings.json"), b"{\"theme\":\"dark\"}");
    assert!(
        Importer::resume_archive(r.engine.clone(), &id, &pass()).is_err(),
        "Done"
    );
}

#[test]
fn resume_from_applying_rolls_back_journal() {
    let _g = lock();
    let sh = shared();
    let r = Side::new(|p| p);
    r.mkdir(".config/app", 0o750);
    r.put(".config/app/notes.txt", b"receiver notes", 0o644);
    let before = r.snap();
    let id = {
        let mut imp = Importer::open(r.engine.clone(), &sh.archive, &pass()).unwrap();
        imp.plan(&confirmed_keys()).unwrap();
        imp.approve().unwrap();
        imp.apply().unwrap();
        imp.id().clone()
    };
    assert_ne!(r.snap(), before);
    // Simulate a crash after the journal was written but before `Done` was
    // persisted: the record still says Applying.
    let store = Store::open(&*r.platform).unwrap();
    let mut rec = store.load(&*r.platform, &id).unwrap();
    rec.state = State::Applying;
    store.save(&*r.platform, &rec).unwrap();
    // A stale staging dir from the crash.
    std::fs::create_dir_all(r.session_dir(&id).join(STAGING_DIR)).unwrap();

    // Dismiss is refused while applying.
    assert!(sessions::dismiss(&r.engine, &id).is_err());

    // Recovery works even with a wrong passphrase (needs no archive).
    let imp =
        Importer::resume_archive(r.engine.clone(), &id, &Passphrase::new("x".into())).unwrap();
    assert_eq!(imp.state(), State::RolledBack);
    assert!(imp.manifest().is_none());
    assert_eq!(r.snap(), before);
    assert!(!r.session_dir(&id).join(STAGING_DIR).exists());
    assert_eq!(
        sessions::show(&r.engine, &id).unwrap().state,
        State::RolledBack
    );
}

#[test]
fn sessions_list_show_dismiss_and_corrupt_records() {
    let _g = lock();
    let s = sender();
    let mut ids = Vec::new();
    for _ in 0..2 {
        let exp = Exporter::start(s.engine.clone(), ScanOptions::default()).unwrap();
        ids.push(exp.id().clone());
    }
    for id in &ids {
        let shape = id.0.split('-').collect::<Vec<_>>();
        assert_eq!(shape.len(), 3, "{id}");
        assert_eq!(shape[0].len(), 8);
        assert_eq!(shape[1].len(), 6);
        assert_eq!(shape[2].len(), 6);
        assert!(shape[2].bytes().all(|b| b.is_ascii_hexdigit()));
    }
    assert_ne!(ids[0], ids[1]);

    // A corrupt record is reported, not fatal.
    let root = Store::open(&*s.platform).unwrap().root().to_path_buf();
    #[cfg(unix)]
    assert_eq!(
        s.platform.metadata(&root).unwrap().mode.unwrap() & 0o777,
        0o700
    );
    std::fs::create_dir(root.join("20000101-000000-badbad")).unwrap();
    std::fs::write(
        root.join("20000101-000000-badbad/session.json"),
        b"{not json",
    )
    .unwrap();
    let list = sessions::list(&s.engine).unwrap();
    assert_eq!(list.len(), 2);
    assert!(list[0].created_at >= list[1].created_at);
    assert!(list.iter().all(|r| r.state == State::Scanned));
    assert!(
        s.sink
            .events()
            .iter()
            .any(|e| matches!(e, Event::Warning { message } if message.contains("badbad")))
    );
    assert!(sessions::show(&s.engine, &SessionId("../escape".into())).is_err());
    assert!(sessions::show(&s.engine, &SessionId("20000101-000000-nosuch".into())).is_err());

    // session.json is 600; dismiss of a session without rollback point is fine.
    #[cfg(unix)]
    let sj = root.join(&ids[0].0).join("session.json");
    #[cfg(unix)]
    assert_eq!(
        s.platform.metadata(&sj).unwrap().mode.unwrap() & 0o777,
        0o600
    );
    sessions::dismiss(&s.engine, &ids[0]).unwrap();
    // Exports cannot be rolled back or resumed.
    assert!(sessions::rollback(&s.engine, Some(&ids[0])).is_err());
    assert!(Importer::resume_archive(s.engine.clone(), &ids[0], &pass()).is_err());
}

/// Re-export the sender's data to `path` (fresh salt → different archive).
fn export_again(path: &Path) {
    let s = sender();
    let mut exp = Exporter::start(
        s.engine.clone(),
        ScanOptions {
            include_undetected: false,
            unrecognised: false,
        },
    )
    .unwrap();
    let sel = exp.suggest(Preset::Suggested);
    exp.select(sel).unwrap();
    exp.export(
        path,
        &pass(),
        WriteOptions {
            kdf: KdfParams::FLOOR,
        },
    )
    .unwrap();
}

#[test]
fn archive_swapped_after_plan_fails_and_writes_nothing() {
    let _g = lock();
    let sh = shared();
    let r = Side::new(|p| p);
    let input = r.home.join("../in.carrova");
    let other = r.home.join("../other.carrova");
    export_again(&other);
    let before = r.snap();

    // Swapped between approval and staging.
    std::fs::copy(&sh.archive, &input).unwrap();
    let mut imp = Importer::open(r.engine.clone(), &input, &pass()).unwrap();
    imp.plan(&confirmed_keys()).unwrap();
    imp.approve().unwrap();
    std::fs::copy(&other, &input).unwrap();
    let err = imp.apply().unwrap_err();
    assert!(
        matches!(&err, CoreError::Verification(m) if m.contains("archive changed since plan")),
        "{err}"
    );
    assert_eq!(imp.state(), State::Failed);
    assert_eq!(
        sessions::show(&r.engine, imp.id()).unwrap().state,
        State::Failed
    );
    assert_eq!(r.snap(), before, "nothing written");
    assert!(!r.session_dir(imp.id()).join(STAGING_DIR).exists());

    // Swapped before a resume: refused, session Failed.
    std::fs::copy(&sh.archive, &input).unwrap();
    let id = {
        let mut imp = Importer::open(r.engine.clone(), &input, &pass()).unwrap();
        imp.plan(&confirmed_keys()).unwrap();
        imp.approve().unwrap();
        imp.id().clone()
    };
    std::fs::copy(&other, &input).unwrap();
    let err = Importer::resume_archive(r.engine.clone(), &id, &pass()).unwrap_err();
    assert!(matches!(err, CoreError::Verification(_)), "{err}");
    assert_eq!(sessions::show(&r.engine, &id).unwrap().state, State::Failed);
    assert_eq!(r.snap(), before);
}
