//! Plugin items end to end: two engines in one process, a Linux sender and
//! a macOS receiver (`with_os`), the real Obsidian recipe and the real
//! `path-rewrite` plugin. The vault list's home paths are rewritten, the
//! plugin needs the user's consent first, and rollback recognises the
//! rewritten file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use carrova_core::plan::{Blocker, Plan};
use carrova_core::{
    CoreError, Engine, Event, Exporter, Importer, KdfParams, Passphrase, PauseReason, Phase,
    PlanOptions, Preset, ScanOptions, State, WriteOptions, sessions,
};
use carrova_platform::OsKind;
use carrova_platform::fake::{FakePlatform, Hooks};
use carrova_plugin::{Plugin, PluginManifest, PluginPackage};
use carrova_recipe::PluginId;
use lan_support::HookSink;
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
    /// Shares hook state with the engine's platform (free-space override).
    platform: FakePlatform,
    sink: Arc<HookSink>,
}

/// A machine on `os`, roots laid out like the real adapter's.
fn side(os: OsKind, plugins: Vec<Plugin>) -> Side {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let platform = FakePlatform::with_os_layout(&home, os)
        .with_state_dir(tmp.path().join("state/carrova"))
        .binary("obsidian", "/usr/bin/obsidian")
        .with_hooks(Hooks::default());
    let sink = Arc::new(HookSink::default());
    let engine = Engine::new(
        Arc::new(platform.clone()),
        common::recipes(RECIPE),
        sink.clone(),
    )
    .with_plugins(plugins);
    Side {
        tmp,
        home,
        engine,
        platform,
        sink,
    }
}

/// A Linux machine holding an Obsidian vault list, everything suggested
/// selected.
fn linux_sender() -> (Side, Exporter) {
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
    (s, exp)
}

/// The Linux sender's archive (built once per test binary) and its home.
fn archive() -> &'static (PathBuf, PathBuf) {
    static ARCHIVE: OnceLock<(Side, (PathBuf, PathBuf))> = OnceLock::new();
    let (_, out) = ARCHIVE.get_or_init(|| {
        let (s, mut exp) = linux_sender();
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

#[test]
fn plugin_items_need_room_for_their_output_on_the_state_dir_filesystem() {
    let (r, mut imp) = receiver(vec![path_rewrite()]);
    let plan = imp.plan(&confirmed()).unwrap().clone();
    let vaults = vault_list(&archive().1).len() as u64;
    let needs =
        carrova_core::plan::space_needs(&**r.engine.platform(), &plan, imp.manifest().unwrap())
            .unwrap();
    let state = needs.iter().find(|n| n.state_dir).unwrap();
    assert_eq!(state.bytes, plan.bytes_to_write + vaults, "{needs:?}");
}

#[test]
fn plugin_output_without_room_is_a_full_disk_not_a_partial_write() {
    let (r, mut imp) = receiver(vec![path_rewrite()]);
    imp.plan(&confirmed()).unwrap();
    imp.approve().unwrap();
    r.platform.override_free_space(Some(1));
    let err = imp.apply().unwrap_err();
    assert!(matches!(err, CoreError::DiskFull { .. }), "{err:?}");
    let target = r
        .home
        .join("Library/Application Support/obsidian/obsidian.json");
    assert!(!target.exists());
}

#[test]
fn a_plan_approved_with_the_space_override_writes_plugin_output_on_a_low_disk() {
    let (r, mut imp) = receiver(vec![path_rewrite()]);
    r.platform.override_free_space(Some(1));
    let options = PlanOptions {
        ignore_free_space: true,
        ..confirmed()
    };
    imp.plan(&options).unwrap();
    imp.approve().unwrap();
    imp.apply().unwrap();
    let target = r
        .home
        .join("Library/Application Support/obsidian/obsidian.json");
    assert_eq!(
        std::fs::read_to_string(target).unwrap(),
        vault_list(&r.home)
    );
}

#[test]
fn plugin_output_without_room_pauses_a_lan_transfer_until_space_frees() {
    let (_s, mut exp) = linux_sender();
    let running = lan_support::run(exp.prepare_send().unwrap());
    let r = side(OsKind::MacOs, vec![path_rewrite()]);
    let mut imp = Importer::connect(r.engine.clone(), running.first.as_ref().unwrap()).unwrap();
    imp.plan(&confirmed()).unwrap();
    imp.approve().unwrap();
    // The disk fills up once every chunk is staged, before the plugin runs.
    let platform = r.platform.clone();
    r.sink.set_hook(move |e| {
        if let Event::Progress {
            phase: Phase::Transfer,
            done,
            total,
        } = e
            && done == total
        {
            platform.override_free_space(Some(1));
        }
    });
    let paused = |e: &Event| {
        matches!(
            e,
            Event::TransferPaused {
                reason: PauseReason::LowDiskSpace { .. }
            }
        )
    };
    let applying = std::thread::spawn(move || imp.apply().map(|_| imp));
    r.sink.wait_for("low-disk pause", paused);
    r.sink.clear_hook();
    r.platform.override_free_space(None);
    let imp = applying.join().unwrap().unwrap();

    assert_eq!(imp.state(), State::Done);
    let events = r.sink.events();
    let at = events.iter().position(paused).unwrap();
    assert!(
        events[at..]
            .iter()
            .any(|e| matches!(e, Event::TransferResumed))
    );
    let target = r
        .home
        .join("Library/Application Support/obsidian/obsidian.json");
    assert_eq!(
        std::fs::read_to_string(target).unwrap(),
        vault_list(&r.home)
    );
    running.thread.join().unwrap().unwrap();
}

/// `recipe_yaml`'s detected items captured on a Linux sender holding
/// `files` (home-relative) and `plugins`, with sender-side export allowed
/// for `allowed`.
fn capture_full(
    recipe_yaml: &str,
    plugins: &[Plugin],
    allowed: &[PluginId],
    files: &[&str],
) -> (Side, carrova_core::capture::Capture) {
    use carrova_core::capture::capture_with_consent;
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
    let cap = capture_with_consent(
        &**platform,
        &recipes,
        plugins,
        allowed,
        &sel,
        &NullSink,
        &cancel,
    )
    .unwrap();
    (s, cap)
}

/// [`capture_full`] with no export allowed; its manifest.
fn capture_with(
    recipe_yaml: &str,
    plugins: &[Plugin],
    files: &[&str],
) -> carrova_manifest::Manifest {
    let (_, cap) = capture_full(recipe_yaml, plugins, &[], files);
    assert!(!cap.manifest.items.is_empty());
    cap.manifest
}

/// A `path-rewrite` stand-in whose `export_transform` replaces each file
/// with its own path (`transform` is the identity).
fn exporting_plugin() -> Plugin {
    let wat = r#"(module
  (memory (export "memory") 1)
  (global $next (mut i32) (i32.const 1024))
  (func (export "alloc") (param $len i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $next))
    (global.set $next (i32.add (local.get $p) (local.get $len)))
    (local.get $p))
  (func $packed (param i32 i32) (result i64)
    (i64.or (i64.shl (i64.extend_i32_u (local.get 0)) (i64.const 32))
            (i64.extend_i32_u (local.get 1))))
  (func (export "transform") (param i32 i32 i32 i32 i32 i32) (result i64)
    (call $packed (local.get 4) (local.get 5)))
  (func (export "export_transform") (param i32 i32 i32 i32 i32 i32) (result i64)
    (call $packed (local.get 2) (local.get 3))))"#;
    let mut manifest = PluginManifest::parse(PLUGIN_YAML).unwrap();
    manifest.export = true;
    manifest.context.clear();
    Plugin::load(PluginPackage {
        manifest,
        wasm: wat::parse_str(wat).unwrap(),
    })
    .unwrap()
}

#[test]
fn an_allowed_export_plugin_changes_files_before_they_are_sent() {
    use carrova_manifest::{Blake3Hash, ChunkSource, EntryKind};
    let files = &[".config/obsidian/obsidian.json"];
    let (s, cap) = capture_full(RECIPE, &[exporting_plugin()], &[plugin_id()], files);
    let item = cap
        .manifest
        .items
        .iter()
        .find(|i| carrova_core::ItemKey::of(i) == VAULTS)
        .unwrap();
    let entry = item
        .entries
        .iter()
        .find(|e| e.rel.as_str() == "obsidian/obsidian.json")
        .unwrap();
    let want = b"obsidian/obsidian.json";
    let EntryKind::File { size, hash, chunks } = &entry.kind else {
        panic!("not a file: {entry:?}");
    };
    assert_eq!((*size, *hash), (want.len() as u64, Blake3Hash::of(want)));
    let mut source = cap.chunk_source(&**s.engine.platform());
    let sent: Vec<u8> = chunks
        .iter()
        .flat_map(|c| source.read_chunk(c).unwrap())
        .collect();
    assert_eq!(sent, want);
}

#[test]
fn an_export_plugin_the_sender_did_not_allow_leaves_its_item_out() {
    let files = &[".config/obsidian/obsidian.json"];
    let (_, cap) = capture_full(RECIPE, &[exporting_plugin()], &[], files);
    assert!(
        !cap.manifest
            .items
            .iter()
            .any(|i| carrova_core::ItemKey::of(i) == VAULTS)
    );
    let failed = cap.failed.iter().find(|f| f.what == VAULTS).unwrap();
    assert!(failed.reason.contains("path-rewrite"), "{failed:?}");
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

/// A Linux sender holding Obsidian's vault list, with an exporting
/// plugin loaded, and its export session.
fn exporting_sender() -> (Side, Exporter) {
    let s = side(OsKind::Linux, vec![exporting_plugin()]);
    let config = s.home.join(".config/obsidian");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("obsidian.json"), vault_list(&s.home)).unwrap();
    let opts = ScanOptions {
        include_undetected: false,
        unrecognised: false,
    };
    let exp = Exporter::start(s.engine.clone(), opts).unwrap();
    (s, exp)
}

fn floor() -> WriteOptions {
    WriteOptions {
        kdf: KdfParams::FLOOR,
    }
}

#[test]
fn an_export_plugin_the_sender_has_not_allowed_blocks_the_export() {
    let (s, mut exp) = exporting_sender();
    let sel = exp.suggest(Preset::Suggested);
    let [planned] = sel.export_plugins.as_slice() else {
        panic!("one exporting plugin expected: {:?}", sel.export_plugins);
    };
    assert_eq!(planned.plugin, plugin_id());
    assert_eq!(planned.items, vec![carrova_core::ItemKey::from(VAULTS)]);
    assert!(!planned.permissions.is_empty());
    let issues = sel.issues();
    let [issue] = issues.as_slice() else {
        panic!("one issue expected: {issues:?}");
    };
    assert_eq!(
        (issue.item.as_str(), &issue.plugin),
        (VAULTS, &Some(plugin_id()))
    );

    let err = exp.select(sel).unwrap_err();
    assert!(matches!(err, CoreError::Selection(_)), "{err:?}");
    let path = s.tmp.path().join("out.carrova");
    assert!(exp.export(&path, &pass(), floor()).is_err());
    assert!(!path.exists(), "nothing may be written without consent");
}

#[test]
fn an_export_plugin_the_sender_allowed_changes_what_the_receiver_gets() {
    let (s, mut exp) = exporting_sender();
    let mut sel = exp.suggest(Preset::Suggested);
    sel.apply(carrova_core::SelectionEdit::AllowPlugin {
        plugin: plugin_id(),
        allow: true,
    })
    .unwrap();
    assert!(sel.issues().is_empty(), "{:?}", sel.issues());
    exp.select(sel).unwrap();
    let path = s.tmp.path().join("out.carrova");
    let (_, checklist) = exp.export(&path, &pass(), floor()).unwrap();
    assert!(checklist.skipped.is_empty(), "{:?}", checklist.skipped);

    let r = side(OsKind::Linux, vec![exporting_plugin()]);
    let mut imp = Importer::open(r.engine.clone(), &path, &pass()).unwrap();
    imp.plan(&confirmed()).unwrap();
    imp.approve().unwrap();
    imp.apply().unwrap();
    let got = std::fs::read_to_string(r.home.join(".config/obsidian/obsidian.json")).unwrap();
    assert_eq!(got, "obsidian/obsidian.json");
}
