//! One small app migrated by archive between two engines in one
//! process: a sender's archive (built once per test binary, floor KDF)
//! and a receiver that applied it.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use carrova_core::store::Store;
use carrova_core::{
    ArchiveSummary, Engine, Exporter, Importer, KdfParams, Passphrase, PlanOptions, Preset,
    ScanOptions, SessionId, SessionRecord, VecSink, WriteOptions, sessions,
};
use carrova_platform::fake::FakePlatform;
use tempfile::TempDir;

use crate::common;

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
";

pub fn pass() -> Passphrase {
    Passphrase::new("correct horse battery staple".into())
}

/// One machine: home + separate state dir.
pub struct Side {
    pub tmp: TempDir,
    pub home: PathBuf,
    pub platform: Arc<FakePlatform>,
    pub engine: Engine,
}

impl Side {
    pub fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(home.join(".config")).unwrap();
        let platform = Arc::new(
            FakePlatform::with_home(&home)
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

    pub fn put(&self, rel: &str, bytes: &[u8]) {
        common::put(&*self.platform, &self.home, rel, bytes, 0o644);
    }

    pub fn session_dir(&self, id: &SessionId) -> PathBuf {
        Store::open(&*self.platform)
            .unwrap()
            .session_dir(id)
            .unwrap()
    }
}

/// The sender's archive and its export session's record.
pub struct Exported {
    pub path: PathBuf,
    pub summary: ArchiveSummary,
    pub record: SessionRecord,
}

/// The sender's archive (built once per test binary).
pub fn archive() -> &'static Exported {
    static ARCHIVE: OnceLock<(Side, Exported)> = OnceLock::new();
    let (_, exported) = ARCHIVE.get_or_init(|| {
        let s = Side::new();
        common::mkdir(&*s.platform, &s.home, ".config/app", 0o755);
        s.put(".config/app/settings.json", b"sender settings");
        s.put(".config/app/new.txt", b"only on the sender");
        let opts = ScanOptions {
            include_undetected: false,
            unrecognised: false,
        };
        let mut exp = Exporter::start(s.engine.clone(), opts).unwrap();
        exp.select(exp.suggest(Preset::Suggested)).unwrap();
        let path = s.tmp.path().join("out.carrova");
        let floor = WriteOptions {
            kdf: KdfParams::FLOOR,
        };
        let (summary, _) = exp.export(&path, &pass(), floor).unwrap();
        let record = sessions::show(&s.engine, exp.id()).unwrap();
        let exported = Exported {
            path,
            summary,
            record,
        };
        (s, exported)
    });
    exported
}

/// A receiver whose `settings.json` the import replaces (backed up) and
/// next to which it creates `new.txt`; returns it with the importer.
pub fn applied() -> (Side, Importer) {
    let r = Side::new();
    common::mkdir(&*r.platform, &r.home, ".config/app", 0o755);
    r.put(".config/app/settings.json", b"receiver settings");
    let mut imp = Importer::open(r.engine.clone(), &archive().path, &pass()).unwrap();
    imp.plan(&PlanOptions::default()).unwrap();
    imp.approve().unwrap();
    imp.apply().unwrap();
    (r, imp)
}
