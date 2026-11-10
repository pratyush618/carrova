use std::collections::BTreeMap;
use std::path::PathBuf;

use carrova_core::plan::{
    Action, Blocker, Handler, ObjectKind, PathMapping, Plan, PlanOptions, PlanWarning,
    PlannedAction, PlannedHandler,
};
use carrova_core::session::SessionId;
use carrova_manifest::{ItemFlags, Manifest, ManifestItem, SourceInfo};
use carrova_platform::{OsKind, SemanticRoot};

use carrova_view::Detail;
const ESC: &str = "\x1b[1A\x1b[2K";

fn action(rel: &str, kind: ObjectKind, action: Action) -> PlannedAction {
    PlannedAction {
        item: "gnupg/keys".into(),
        entry_index: 0,
        kind,
        root: SemanticRoot::Home,
        rel: rel.into(),
        abs: PathBuf::from("/h").join(rel),
        size: 3,
        mode: (kind != ObjectKind::Symlink).then_some(0o600),
        link_target: None,
        mtime_ns: None,
        action,
    }
}

fn manifest_item(recipe: &str, id: &str, label: &str) -> ManifestItem {
    ManifestItem {
        recipe_id: Some(recipe.into()),
        item_id: id.into(),
        label: label.into(),
        flags: ItemFlags::default(),
        entries: Vec::new(),
        handler: None,
        artifacts: Vec::new(),
        source_paths: Vec::new(),
    }
}

fn sample() -> (Plan, Manifest) {
    let mut link = action(".gnupg/cur", ObjectKind::Symlink, Action::Create);
    link.link_target = Some(format!("keys{ESC}v2"));
    let plan = Plan {
        session: SessionId("20260101-000000-abcdef".into()),
        options: PlanOptions::default(),
        actions: vec![
            action(".gnupg/gpg.conf", ObjectKind::File, Action::Create),
            action(".gnupg", ObjectKind::Dir, Action::Replace),
            link,
            action(&format!("x{ESC}"), ObjectKind::File, Action::Replace),
            action(
                ".gnupg/old",
                ObjectKind::File,
                Action::Skip {
                    reason: format!("rule{ESC}"),
                },
            ),
        ],
        handlers: vec![PlannedHandler {
            item: "gnupg/keys".into(),
            handler: Handler::GpgKeyring,
            backup: Vec::new(),
            import_trust: false,
        }],
        bytes_to_write: 9,
        free_space: None,
        missing_apps: vec![format!("app{ESC}")],
        running_apps: Vec::new(),
        app_names: Default::default(),
        mappings: Vec::new(),
        plugins: Vec::new(),
        blockers: vec![Blocker::RunningApp {
            app: format!("app{ESC}"),
            name: format!("app{ESC}"),
        }],
        warnings: vec![PlanWarning {
            item: Some(format!("i{ESC}").into()),
            message: format!("w{ESC}"),
        }],
        sensitive: BTreeMap::from([("gnupg/keys".into(), "holds private keys".to_owned())]),
    };
    let manifest = Manifest::new(
        0,
        SourceInfo {
            os: OsKind::Linux,
            carrova_version: "0".into(),
            device_name: None,
        },
        vec![
            manifest_item("gnupg", "keys", "GnuPG keys"),
            manifest_item("git", "config", &format!("Git{ESC}home:.bashrc")),
        ],
    );
    (plan, manifest)
}

fn render(plan: &Plan, manifest: &Manifest) -> String {
    render_at(plan, manifest, Detail::Summary)
}

fn render_at(plan: &Plan, manifest: &Manifest, detail: Detail) -> String {
    let mut out = Vec::new();
    super::plan(&mut out, plan, Some(manifest), &[], detail).unwrap();
    String::from_utf8(out).unwrap()
}

/// `sample()` plus 30 created files under one plugin directory.
fn with_many_creates() -> (Plan, Manifest) {
    let (mut plan, manifest) = sample();
    for i in 0..30 {
        plan.actions.push(action(
            &format!(".gnupg/plugins/big/f{i}"),
            ObjectKind::File,
            Action::Create,
        ));
    }
    (plan, manifest)
}

#[test]
fn default_plan_collapses_dir_with_many_creates() {
    let (plan, manifest) = with_many_creates();
    let out = render(&plan, &manifest);
    assert!(
        out.contains("    + home:.gnupg/  (31 files, 93 B)"),
        "{out}"
    );
    assert!(!out.contains("f17"), "{out}");
    assert!(out.contains("-v lists every path"), "{out}");
    assert!(
        out.contains(
            "gnupg/keys  create 32 \u{b7} replace 2 \u{b7} keep-both 0 \u{b7} skip 1 \u{b7} identical 0"
        ),
        "{out}"
    );
}

#[test]
fn verbose_plan_lists_every_path() {
    let (plan, manifest) = with_many_creates();
    let out = render_at(&plan, &manifest, Detail::Full);
    for i in 0..30 {
        let want = format!("+ home:.gnupg/plugins/big/f{i}  0600  3 B");
        assert!(out.contains(&want), "missing {want:?} in\n{out}");
    }
    assert!(!out.contains("-v lists every path"), "{out}");
}

#[test]
fn replaces_skips_and_symlinks_always_listed() {
    let (plan, manifest) = with_many_creates();
    for detail in [Detail::Summary, Detail::Full] {
        let out = render_at(&plan, &manifest, detail);
        assert!(out.contains("    set mode home:.gnupg/  0600"), "{out}");
        assert!(out.contains("    skip home:.gnupg/old  (rule"), "{out}");
        assert!(out.contains("    + home:.gnupg/cur  -> keys"), "{out}");
    }
}

#[test]
fn dir_replace_shows_mode_change_not_backup() {
    let (plan, m) = sample();
    let out = render(&plan, &m);
    let line = out
        .lines()
        .find(|l| l.contains("home:.gnupg/") && !l.contains("gpg.conf"))
        .unwrap_or_default();
    assert!(line.contains("set mode"), "{line}");
    assert!(!line.contains("backed up"), "{line}");
}

#[test]
fn keep_both_shows_new_name() {
    let (mut plan, manifest) = sample();
    plan.actions.push(action(
        ".gnupg/a.conf",
        ObjectKind::File,
        Action::KeepBoth {
            rel: ".gnupg/a.conf.carrova-20260101".into(),
        },
    ));
    let out = render(&plan, &manifest);
    assert!(
        out.contains("keep-both home:.gnupg/a.conf  -> home:.gnupg/a.conf.carrova-20260101"),
        "{out}"
    );
}

#[test]
fn plan_warnings_printed_once() {
    let (plan, manifest) = sample();
    let out = render(&plan, &manifest);
    assert_eq!(out.matches("w\\u{1b}").count(), 1, "{out}");
}

#[test]
fn keep_group_write_shown_in_header_only_when_on() {
    let (mut plan, manifest) = sample();
    assert!(!render(&plan, &manifest).contains("group-write"));
    plan.options.keep_group_write = true;
    let out = render(&plan, &manifest);
    assert!(
        out.contains("option: keep group-write bits from the archive"),
        "{out}"
    );
}

#[test]
fn plan_output_escapes_control_chars() {
    let (plan, manifest) = sample();
    let out = render(&plan, &manifest);
    assert!(!out.contains('\x1b'), "{out}");
    for want in [
        "home:x\\u{1b}[1A\\u{1b}[2K",
        "rule\\u{1b}",
        "Git\\u{1b}[1A\\u{1b}[2Khome:.bashrc",
        "-> keys\\u{1b}[1A\\u{1b}[2Kv2",
        "app\\u{1b}",
        "(`app\\u{1b}[1A\\u{1b}[2K`) is running",
        "i\\u{1b}[1A\\u{1b}[2K: w\\u{1b}",
    ] {
        assert!(out.contains(want), "missing {want:?} in\n{out}");
    }
}

#[test]
fn plan_output_shows_modes_and_link_targets() {
    let (plan, manifest) = sample();
    let out = render(&plan, &manifest);
    let line = |needle: &str| out.lines().find(|l| l.contains(needle)).unwrap().to_owned();
    assert!(line("home:.gnupg/gpg.conf").contains("0600"), "{out}");
    assert!(line("home:.gnupg/ ").contains("0600"), "{out}");
    assert!(line("home:.gnupg/cur").contains("-> keys"), "{out}");
}

#[test]
fn receiver_sensitive_items_marked_with_reason() {
    let (plan, manifest) = sample();
    let out = render(&plan, &manifest);
    let line = out.lines().find(|l| l.contains("GnuPG keys")).unwrap();
    assert!(line.starts_with("  [ ]"), "{out}");
    assert!(line.contains("holds private keys"), "{out}");
    assert!(line.contains("--confirm-sensitive gnupg/keys"), "{out}");

    let mut confirmed = plan.clone();
    confirmed.options.confirmed_sensitive = vec!["gnupg/keys".into()];
    let out = render(&confirmed, &manifest);
    let line = out.lines().find(|l| l.contains("GnuPG keys")).unwrap();
    assert!(line.starts_with("  [x]"), "{out}");
    assert!(line.contains("confirmed"), "{out}");
}

#[test]
fn handler_trust_import_state_shown() {
    let (mut plan, manifest) = sample();
    let out = render(&plan, &manifest);
    assert!(out.contains("trust import off"), "{out}");
    plan.handlers[0].import_trust = true;
    plan.options.import_trust = true;
    let out = render(&plan, &manifest);
    assert!(out.contains("trust import on"), "{out}");
}

#[test]
fn cross_os_mappings_and_blocker_hint_are_shown() {
    let (mut plan, manifest) = sample();
    plan.mappings = vec![PathMapping {
        item: "git/config".into(),
        from: (SemanticRoot::UserConfig, format!("Code{ESC}/User")),
        to: (SemanticRoot::UserData, "Code/User".into()),
        to_abs: None,
    }];
    plan.blockers = vec![Blocker::CrossOs {
        item: "gnupg/keys".into(),
        from: OsKind::Linux,
        to: OsKind::Windows,
    }];
    let out = render(&plan, &manifest);
    assert!(out.contains("Mapped to this OS's locations:"), "{out}");
    assert!(
        out.contains("  git/config  user_config:Code\\u{1b}"),
        "{out}"
    );
    assert!(out.contains("-> user_data:Code/User"), "{out}");
    assert!(out.contains("(--allow-cross-os gnupg/keys)"), "{out}");
}

#[test]
fn plugins_show_their_permissions_and_the_allow_hint() {
    let (mut plan, manifest) = sample();
    let plugin = carrova_recipe::PluginId::new("path-rewrite").unwrap();
    plan.plugins = vec![carrova_core::plan::PlannedPlugin {
        plugin: plugin.clone(),
        version: "1.0.0".into(),
        description: format!("Rewrites paths{ESC}."),
        permissions: vec!["It sees the path of your home folder on this computer.".into()],
        items: vec!["obsidian/vaults".into()],
    }];
    plan.blockers = vec![Blocker::UnconfirmedPlugin {
        plugin,
        items: vec!["obsidian/vaults".into()],
    }];
    let out = render(&plan, &manifest);
    assert!(
        out.contains("Plugin: path-rewrite 1.0.0 changes obsidian/vaults: Rewrites paths\\u{1b}"),
        "{out}"
    );
    assert!(
        out.contains("    - It sees the path of your home folder on this computer."),
        "{out}"
    );
    assert!(out.contains("(--allow-plugin path-rewrite)"), "{out}");
}

#[test]
fn space_line_shows_free_of_total_when_capacity_is_known() {
    let (plan, manifest) = sample();
    let need = |total| carrova_core::plan::SpaceNeed {
        state_dir: true,
        roots: vec![SemanticRoot::Home],
        bytes: 99,
        needed: 500,
        available: Some(2_000),
        total,
    };
    let mut out = Vec::new();
    super::plan(
        &mut out,
        &plan,
        Some(&manifest),
        &[need(Some(8_000))],
        Detail::Summary,
    )
    .unwrap();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("free 2.0 kB of 8.0 kB"), "{s}");
    let mut out = Vec::new();
    super::plan(
        &mut out,
        &plan,
        Some(&manifest),
        &[need(None)],
        Detail::Summary,
    )
    .unwrap();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("free 2.0 kB\n"), "{s}");
}
