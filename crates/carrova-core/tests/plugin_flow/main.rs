//! Plugin items end to end: two engines in one process, a Linux sender and
//! a macOS receiver (`with_os`), the real Obsidian recipe and the real
//! `path-rewrite` plugin. The vault list's home paths are rewritten, the
//! plugin needs the user's consent first, and rollback recognises the
//! rewritten file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use carrova_core::plan::{Blocker, Plan};
use carrova_core::{
    Engine, Exporter, Importer, KdfParams, Passphrase, PlanOptions, Preset, ScanOptions, VecSink,
    WriteOptions, sessions,
};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{OsKind, SemanticRoot};
use carrova_plugin::{Plugin, PluginManifest, PluginPackage};
use carrova_recipe::PluginId;
use tempfile::TempDir;

const RECIPE: &str = include_str!("../../../../recipes/notes/obsidian/recipe.yaml");
const PLUGIN_YAML: &str = include_str!("../../../../plugins/path-rewrite/plugin.yaml");
const PLUGIN_WASM: &[u8] = include_bytes!("../../../../plugins/path-rewrite/plugin.wasm");
const VAULTS: &str = "obsidian/vaults";

fn pass() -> Passphrase {
    Passphrase::new("correct horse battery staple".into())
}

fn path_rewrite() -> Plugin {
    Plugin::load(PluginPackage {
        manifest: PluginManifest::parse(PLUGIN_YAML).unwrap(),
        wasm: PLUGIN_WASM.to_vec(),
    })
    .unwrap()
}

fn plugin_id() -> PluginId {
    PluginId::new("path-rewrite").unwrap()
}

/// `{"vaults":{…:{"path":"<home>/Notes",…}}}`.
/// `home` as it appears inside a JSON string (backslashes escaped on Windows).
fn json_spelled(home: &Path) -> String {
    let quoted = serde_json::to_string(&home.display().to_string()).unwrap();
    quoted[1..quoted.len() - 1].to_owned()
}

/// Obsidian's vault list, which stores absolute paths as JSON strings.
fn vault_list(home: &Path) -> String {
    format!(
        r#"{{"vaults":{{"0123456789abcdef":{{"path":"{}/Notes","ts":1,"open":true}}}}}}"#,
        json_spelled(home)
    )
}

struct Side {
    tmp: TempDir,
    home: PathBuf,
    engine: Engine,
}

/// A machine on `os`; macOS keeps `user_config` in `Library/Application
/// Support` like the real adapter.
fn side(os: OsKind, plugins: Vec<Plugin>) -> Side {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let mut platform = FakePlatform::with_home(&home)
        .with_state_dir(tmp.path().join("state/carrova"))
        .with_os(os)
        .binary("obsidian", "/usr/bin/obsidian");
    if os == OsKind::MacOs {
        platform = platform.root(
            SemanticRoot::UserConfig,
            home.join("Library/Application Support"),
        );
    }
    let engine = Engine::new(
        Arc::new(platform),
        common::recipes(RECIPE),
        Arc::new(VecSink::default()),
    )
    .with_plugins(plugins);
    Side { tmp, home, engine }
}

/// The Linux sender's archive (built once per test binary) and its home.
fn archive() -> &'static (PathBuf, PathBuf) {
    static ARCHIVE: OnceLock<(Side, (PathBuf, PathBuf))> = OnceLock::new();
    let (_, out) = ARCHIVE.get_or_init(|| {
        let s = side(OsKind::Linux, vec![path_rewrite()]);
        let config = s.home.join(".config/obsidian");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(config.join("obsidian.json"), vault_list(&s.home)).unwrap();
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
        exp.export(&path, &pass(), floor).unwrap();
        let home = s.home.clone();
        (s, (path, home))
    });
    out
}

fn receiver(plugins: Vec<Plugin>) -> (Side, Importer) {
    let r = side(OsKind::MacOs, plugins);
    let imp = Importer::open(r.engine.clone(), &archive().0, &pass()).unwrap();
    (r, imp)
}

fn confirmed() -> PlanOptions {
    PlanOptions {
        confirmed_plugins: vec![plugin_id()],
        ..PlanOptions::default()
    }
}

fn written(p: &Plan) -> Vec<&str> {
    p.actions
        .iter()
        .filter(|a| a.item == VAULTS && !a.action.is_unchanged())
        .map(|a| a.rel.as_str())
        .collect()
}

#[test]
fn a_plugin_item_is_blocked_until_the_user_allows_the_plugin() {
    let (_r, mut imp) = receiver(vec![path_rewrite()]);
    let plan = imp.plan(&PlanOptions::default()).unwrap();
    assert_eq!(
        plan.blockers,
        [Blocker::UnconfirmedPlugin {
            plugin: plugin_id(),
            items: vec![VAULTS.into()],
        }]
    );
    let [planned] = plan.plugins.as_slice() else {
        panic!("one planned plugin expected: {:?}", plan.plugins);
    };
    assert_eq!(planned.plugin, plugin_id());
    assert_eq!(planned.items, vec![carrova_core::ItemKey::from(VAULTS)]);
    assert!(
        planned
            .permissions
            .iter()
            .any(|p| p.contains("home folder on the old computer")),
        "{:?}",
        planned.permissions
    );

    let plan = imp.plan(&confirmed()).unwrap();
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    assert_eq!(written(plan), ["obsidian/obsidian.json"]);
}

#[test]
fn a_missing_plugin_skips_the_item_with_a_warning() {
    let (_r, mut imp) = receiver(Vec::new());
    let plan = imp.plan(&PlanOptions::default()).unwrap();
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    assert!(plan.plugins.is_empty());
    assert!(written(plan).is_empty(), "{:?}", plan.actions);
    assert!(
        plan.warnings
            .iter()
            .any(|w| w.item.as_ref().is_some_and(|i| i == VAULTS)
                && w.message.contains("path-rewrite")),
        "{:?}",
        plan.warnings
    );
}

#[test]
fn linux_vault_paths_are_rewritten_for_macos_and_rolled_back() {
    let (r, mut imp) = receiver(vec![path_rewrite()]);
    imp.plan(&confirmed()).unwrap();
    imp.approve().unwrap();
    imp.apply().unwrap();

    let target = r
        .home
        .join("Library/Application Support/obsidian/obsidian.json");
    let text = std::fs::read_to_string(&target).unwrap();
    assert_eq!(text, vault_list(&r.home));
    assert!(!text.contains(&json_spelled(&archive().1)), "{text}");

    let report = sessions::rollback(&r.engine, Some(imp.id())).unwrap();
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert!(!target.exists(), "rollback must remove the rewritten file");
}

/// `recipe_yaml`'s detected items captured on a Linux sender holding
/// `files` (home-relative) and `plugins`.
fn capture_with(
    recipe_yaml: &str,
    plugins: &[Plugin],
    files: &[&str],
) -> carrova_manifest::Manifest {
    use carrova_core::capture::capture_with_plugins;
    use carrova_core::{Cancel, NullSink, Selection, scan};
    let s = side(OsKind::Linux, Vec::new());
    for rel in files {
        let path = s.home.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, vault_list(&s.home)).unwrap();
    }
    let recipes = common::recipes(recipe_yaml);
    let platform = s.engine.platform();
    let report = scan(&recipes, &**platform, ScanOptions::default()).unwrap();
    let sel = Selection::from_scan(&report, Preset::Suggested);
    let cancel = Cancel::default();
    let cap =
        capture_with_plugins(&**platform, &recipes, plugins, &sel, &NullSink, &cancel).unwrap();
    assert!(!cap.manifest.items.is_empty());
    cap.manifest
}

#[test]
fn the_sender_home_is_recorded_only_when_an_items_plugin_declares_it() {
    let files = &[".config/obsidian/obsidian.json"];
    let plain = RECIPE.replace("    handler: { plugin: path-rewrite }\n", "");
    let no_plugin_item = capture_with(&plain, &[path_rewrite()], files);
    assert_eq!(no_plugin_item.source_home, None);
    let plugin_not_here = capture_with(RECIPE, &[], files);
    assert_eq!(plugin_not_here.source_home, None);
    let undeclared = Plugin::load(PluginPackage {
        manifest: PluginManifest::parse(&PLUGIN_YAML.replace("source_home, ", "")).unwrap(),
        wasm: PLUGIN_WASM.to_vec(),
    })
    .unwrap();
    assert_eq!(capture_with(RECIPE, &[undeclared], files).source_home, None);
    let declared = capture_with(RECIPE, &[path_rewrite()], files);
    assert!(declared.source_home.is_some());
}

#[test]
fn a_plugin_needing_the_sender_home_is_skipped_without_it() {
    let mut m = capture_with(
        RECIPE,
        &[path_rewrite()],
        &[".config/obsidian/obsidian.json"],
    );
    m.source_home = None;
    let r = side(OsKind::MacOs, Vec::new());
    let plan = carrova_core::plan::plan_with_plugins(
        &**r.engine.platform(),
        r.engine.recipes(),
        &[path_rewrite()],
        &m,
        &confirmed(),
        &carrova_core::SessionId("20261110-000000-abcdef".into()),
        "20261110",
    )
    .unwrap();
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    assert!(plan.plugins.is_empty(), "{:?}", plan.plugins);
    assert!(written(&plan).is_empty(), "{:?}", plan.actions);
    assert!(
        plan.warnings
            .iter()
            .any(|w| w.message.contains("did not record its home folder")),
        "{:?}",
        plan.warnings
    );
}
