use std::fs;
use std::path::{Path, PathBuf};

use carrova_platform::fake::{FakePlatform, symlink};
use carrova_platform::{
    FileMeta, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot, SnapshotCapability,
};
use carrova_recipe::{Recipe, load_str};
use tempfile::TempDir;

use super::*;

const ALPHA: &str = r"
schema: 1
id: alpha
name: Alpha
category: dev
detect:
  linux:
    - binary: alpha
items:
  - id: config
    label: Alpha config
    paths:
      - { root: user_config, rel: alpha }
    flags: { requires_closed: true }
  - id: profile
    label: Alpha profile
    paths:
      - { root: user_data, rel: alpha/profile }
  - id: key
    label: Alpha key
    paths:
      - { root: home, rel: .alpha-key }
    flags: { sensitive: true }
";

const BETA: &str = r"
schema: 1
id: beta
name: Beta
category: dev
detect:
  linux:
    - path: { root: user_config, rel: beta }
items:
  - id: config
    label: Beta config
    paths:
      - { root: user_config, rel: beta }
    flags: { default: false }
";

const GAMMA: &str = r"
schema: 1
id: gamma
name: Gamma
category: apps
detect:
  linux:
    - binary: gamma
items:
  - id: config
    label: Gamma config
    paths:
      - { root: user_config, rel: gamma }
";

const LONG: &str = r"
schema: 1
id: long
name: Long
category: dev
detect:
  linux:
    - path: { root: user_config, rel: long-named-application }
    - binary: long-named-application
items:
  - id: config
    label: Long config
    paths:
      - { root: user_config, rel: long-named-application }
    flags: { requires_closed: true }
";

fn recipe(text: &str) -> Recipe {
    load_str(text, Path::new("inline.yaml")).unwrap()
}

fn recipes() -> Vec<Recipe> {
    vec![recipe(BETA), recipe(GAMMA), recipe(ALPHA)]
}

fn write(path: PathBuf, len: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![b'x'; len]).unwrap();
}

/// Fake home with claimed, unclaimed and symlinked data.
fn home() -> TempDir {
    let tmp = TempDir::new().unwrap();
    let h = tmp.path();
    let cfg = h.join(".config");
    let data = h.join(".local/share");
    write(cfg.join("alpha/a.conf"), 10);
    write(cfg.join("alpha/sub/b.conf"), 5);
    write(data.join("alpha/profile/p"), 7);
    write(data.join("alpha/other/x"), 3); // claimed: item path lies inside `alpha`
    write(cfg.join("beta/x"), 1);
    write(cfg.join("gamma/g"), 4); // gamma not detected -> unrecognised
    write(cfg.join("unknown/f1"), 100);
    write(cfg.join("unknown/deep/f2"), 50);
    write(cfg.join("topfile"), 9); // top-level file: not reported
    write(data.join("stuff/s"), 20);
    write(h.join("outside/big"), 1000);
    symlink(h.join("outside"), cfg.join("linkdir")).unwrap(); // symlinked child: not reported
    write(cfg.join("withlink/real"), 2);
    symlink(h.join("outside"), cfg.join("withlink/l")).unwrap(); // not followed
    tmp
}

fn fake(home: &Path) -> FakePlatform {
    FakePlatform::with_home(home)
        .binary("alpha", "/usr/bin/alpha")
        .free_space_bytes(4242)
}

fn app<'a>(r: &'a ScanReport, id: &str) -> &'a AppReport {
    r.apps.iter().find(|a| a.id == id).expect("app in report")
}

fn item<'a>(a: &'a AppReport, id: &str) -> &'a ItemReport {
    a.items.iter().find(|i| i.id == id).expect("item in app")
}

#[test]
fn detects_apps_and_sizes_items() {
    let h = home();
    let r = scan(&recipes(), &fake(h.path()), ScanOptions::default()).unwrap();

    let ids: Vec<&str> = r.apps.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        ids,
        ["alpha", "beta"],
        "only detected, sorted by (category, id)"
    );

    let alpha = app(&r, "alpha");
    assert!(alpha.detected);
    assert_eq!(alpha.evidence, ["binary `alpha` at /usr/bin/alpha"]);
    let item_ids: Vec<&str> = alpha.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(item_ids, ["config", "profile", "key"], "recipe order");

    let config = item(alpha, "config");
    assert_eq!((config.size, config.file_count), (15, 2));
    assert!(config.flags.requires_closed && config.flags.default_selected);
    assert!(!config.app_running);

    let profile = item(alpha, "profile");
    assert_eq!((profile.size, profile.file_count), (7, 1));

    let key = item(alpha, "key");
    assert_eq!((key.size, key.file_count), (0, 0));
    assert_eq!(key.missing, ["home:.alpha-key"]);
    assert!(key.flags.sensitive && !key.flags.default_selected);

    let beta = app(&r, "beta");
    assert_eq!(beta.evidence, ["dir user_config:beta"]);
    assert!(!item(beta, "config").flags.default_selected);

    assert_eq!(r.platform.os, OsKind::Linux);
    assert_eq!(r.platform.free_space, Some(4242));
    assert_eq!(r.platform.snapshot, Some(Snapshot::None));
    assert_eq!(
        r.platform.roots[&SemanticRoot::Home],
        h.path().to_string_lossy()
    );
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn reports_unclaimed_dirs_only() {
    let h = home();
    let r = scan(&recipes(), &fake(h.path()), ScanOptions::default()).unwrap();
    let raw: Vec<(SemanticRoot, &str, u64, u64)> = r
        .unrecognised
        .iter()
        .map(|i| (i.root, i.rel.as_str(), i.size, i.file_count))
        .collect();
    assert_eq!(
        raw,
        [
            (SemanticRoot::UserConfig, "gamma", 4, 1),
            (SemanticRoot::UserConfig, "unknown", 150, 2),
            // symlink `l` -> outside/big is not followed
            (SemanticRoot::UserConfig, "withlink", 2, 1),
            (SemanticRoot::UserData, "stuff", 20, 1),
        ]
    );
    assert!(r.unrecognised.iter().all(|i| i.unreadable == 0));
}

#[test]
fn item_under_another_root_claims_child() {
    let h = home();
    let text = r"
schema: 1
id: viahome
name: Via home
category: dev
detect:
  linux:
    - path: { root: home, rel: .config/unknown/deep }
items:
  - id: deep
    label: Deep
    paths:
      - { root: home, rel: .config/unknown/deep }
";
    let r = scan(&[recipe(text)], &fake(h.path()), ScanOptions::default()).unwrap();
    assert!(r.unrecognised.iter().all(|i| i.rel != "unknown"));
    assert!(r.unrecognised.iter().any(|i| i.rel == "gamma"));
}

#[test]
fn totals_add_up() {
    let h = home();
    let r = scan(&recipes(), &fake(h.path()), ScanOptions::default()).unwrap();
    // alpha config 15 + profile 7 selected; beta 1 off; key 0.
    assert_eq!(r.totals.default_selected_size, 22);
    assert_eq!(r.totals.default_selected_files, 3);
    assert_eq!(r.totals.recognised_size, 23);
    assert_eq!(r.totals.unrecognised_size, 176);
    assert_eq!(r.totals.total_size, 199);
}

#[test]
fn running_app_marks_requires_closed_items() {
    let h = home();
    let p = fake(h.path()).process(7, "alpha");
    let r = scan(&recipes(), &p, ScanOptions::default()).unwrap();
    let alpha = app(&r, "alpha");
    assert!(alpha.running);
    assert!(item(alpha, "config").app_running);
    assert!(!item(alpha, "profile").app_running, "not requires_closed");
    assert!(
        !app(&r, "beta").running,
        "path-only detection never running"
    );
}

#[test]
fn truncated_process_name_matches() {
    let h = home();
    write(h.path().join(".config/long-named-application/c"), 1);
    let p = fake(h.path()).process(9, "long-named-appl");
    let r = scan(&[recipe(LONG)], &p, ScanOptions::default()).unwrap();
    assert!(item(app(&r, "long"), "config").app_running);

    let p = fake(h.path()).process(9, "long-named");
    let r = scan(&[recipe(LONG)], &p, ScanOptions::default()).unwrap();
    assert!(!app(&r, "long").running, "short prefix is not a truncation");
}

#[test]
fn include_undetected_lists_apps_without_items() {
    let h = home();
    let opts = ScanOptions {
        include_undetected: true,
        ..ScanOptions::default()
    };
    let r = scan(&recipes(), &fake(h.path()), opts).unwrap();
    let ids: Vec<(&str, bool)> = r.apps.iter().map(|a| (a.id.as_str(), a.detected)).collect();
    assert_eq!(ids, [("gamma", false), ("alpha", true), ("beta", true)]);
    let gamma = app(&r, "gamma");
    assert!(gamma.items.is_empty() && gamma.evidence.is_empty());
    // Undetected apps never claim data.
    assert!(r.unrecognised.iter().any(|i| i.rel == "gamma"));
}

#[test]
fn unrecognised_scan_can_be_disabled() {
    let h = home();
    let opts = ScanOptions {
        unrecognised: false,
        ..ScanOptions::default()
    };
    let r = scan(&recipes(), &fake(h.path()), opts).unwrap();
    assert!(r.unrecognised.is_empty());
    assert_eq!(r.totals.unrecognised_size, 0);
}

#[test]
fn json_shape() {
    let h = home();
    let r = scan(&recipes(), &fake(h.path()), ScanOptions::default()).unwrap();
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["platform"]["os"], "linux");
    assert!(v["platform"]["roots"]["user_config"].is_string());
    assert_eq!(v["apps"][0]["items"][0]["flags"]["portability"], "same_os");
    assert_eq!(v["unrecognised"][0]["root"], "user_config");
    assert_eq!(v["totals"]["total_size"], 199);
}

/// Wraps [`FakePlatform`] to inject failures.
#[derive(Debug)]
struct Faulty {
    inner: FakePlatform,
    no_home: bool,
}

impl Platform for Faulty {
    fn os(&self) -> OsKind {
        self.inner.os()
    }
    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        if self.no_home && root == SemanticRoot::Home {
            return Err(PlatformError::RootUnavailable {
                root,
                reason: "HOME unset".into(),
            });
        }
        self.inner.resolve_root(root)
    }
    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        self.inner.find_binary(name)
    }
    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        self.inner.metadata(path)
    }
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        Err(PlatformError::Unsupported("process listing"))
    }
    fn free_space(&self, _path: &Path) -> Result<u64, PlatformError> {
        Err(PlatformError::Unsupported("free space"))
    }
    fn snapshot_capability(&self, path: &Path) -> Result<SnapshotCapability, PlatformError> {
        self.inner.snapshot_capability(path)
    }
}

#[test]
fn platform_failures_degrade_to_warnings() {
    let h = home();
    let p = Faulty {
        inner: fake(h.path()).process(1, "alpha"),
        no_home: false,
    };
    let r = scan(&recipes(), &p, ScanOptions::default()).unwrap();
    assert!(!app(&r, "alpha").running);
    assert_eq!(r.platform.free_space, None);
    assert_eq!(r.warnings.len(), 2, "{:?}", r.warnings);
}

#[test]
fn missing_home_is_fatal() {
    let h = home();
    let p = Faulty {
        inner: fake(h.path()),
        no_home: true,
    };
    assert!(matches!(
        scan(&recipes(), &p, ScanOptions::default()),
        Err(ScanError::HomeUnavailable(_))
    ));
}

#[test]
fn empty_home_scans_cleanly() {
    let tmp = TempDir::new().unwrap();
    let r = scan(&recipes(), &fake(tmp.path()), ScanOptions::default()).unwrap();
    assert_eq!(app(&r, "alpha").items.len(), 3);
    assert!(r.unrecognised.is_empty());
    assert!(r.warnings.is_empty());
}

#[test]
fn report_names_the_os_by_its_stable_id() {
    let tmp = TempDir::new().unwrap();
    let mut r = scan(&recipes(), &fake(tmp.path()), ScanOptions::default()).unwrap();
    r.platform.os = OsKind::MacOs;
    let json = serde_json::to_value(&r.platform).unwrap();
    assert_eq!(json["os"], "macos");
}
