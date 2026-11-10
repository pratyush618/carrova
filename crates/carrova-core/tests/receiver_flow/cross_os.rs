//! Cross-OS restore: a Linux capture planned on a macOS or Windows
//! receiver (two fake engines in one process, the receiver `with_os`).

use std::collections::BTreeSet;

use carrova_core::capture::capture;
use carrova_core::plan::{Blocker, PathMapping};
use carrova_core::select::{Preset, Selection};
use carrova_core::{ScanOptions, scan};

use super::*;

const EDITOR: &str = r"
schema: 3
id: editor
name: Editor
category: dev
detect:
  linux: [{ binary: editor }]
  macos: [{ binary: editor }]
  windows: [{ binary: editor }]
items:
  - id: settings
    label: Settings
    flags: { portability: cross_os }
    paths:
      - { root: user_config, rel: Editor/User }
      - { root: home, rel: .editor }
    os:
      macos:
        paths:
          - { root: user_data, rel: Editor/User }
          - { root: home, rel: .editor }
      windows:
        paths:
          - { root: user_config, rel: EditorWin/User }
          - { root: home, rel: .editor }
  - id: state
    label: State
    paths:
      - { root: user_config, rel: Editor/state }
";

const SETTINGS: &str = "editor/settings";
const STATE: &str = "editor/state";

fn editor() -> Recipe {
    carrova_recipe::load_str(EDITOR, Path::new("editor/recipe.yaml")).unwrap()
}

/// Sender files (the standard ones plus `extra`), then [`capture_linux`].
fn linux_capture(e: &Env, extra: &[&str]) -> Manifest {
    seed(e, extra);
    capture_linux(e)
}

fn seed(e: &Env, extra: &[&str]) {
    for rel in [
        ".config/Editor/User/settings.json",
        ".config/Editor/User/snippets/a.json",
        ".editor/ext.txt",
        ".config/Editor/state/db",
    ]
    .iter()
    .chain(extra)
    {
        put(&e.sender, &e.sender_home, rel, b"x", 0o644);
    }
}

/// Everything selected, captured on Linux by the real capture path.
fn capture_linux(e: &Env) -> Manifest {
    let sender = e.sender.clone().binary("editor", "/usr/bin/editor");
    let recipes = [editor()];
    let report = scan(&recipes, &sender, ScanOptions::default()).unwrap();
    let mut sel = Selection::from_scan(&report, Preset::None);
    sel.set_included(SETTINGS, true).unwrap();
    sel.set_included(STATE, true).unwrap();
    let cap = capture(&sender, &recipes, &sel, &NullSink, &Cancel::default()).unwrap();
    assert!(cap.failed.is_empty(), "{:?}", cap.failed);
    assert_eq!(cap.manifest.source.os, OsKind::Linux);
    cap.manifest
}

fn only(keys: &[&str]) -> PlanOptions {
    PlanOptions {
        items: Some(keys.iter().map(|&k| k.into()).collect()),
        ..PlanOptions::default()
    }
}

fn plan_on(recv: &FakePlatform, recipes: &[Recipe], m: &Manifest, o: &PlanOptions) -> Plan {
    plan_on_date(recv, recipes, m, o, &sid(), DATE).unwrap()
}

/// `root:rel` of every action of `item` that writes something.
fn written(p: &Plan, item: &str) -> BTreeSet<String> {
    p.actions
        .iter()
        .filter(|a| a.item == item && !matches!(a.action, Action::Skip { .. }))
        .map(|a| format!("{}:{}", a.root, a.rel))
        .collect()
}

fn mappings(p: &Plan) -> Vec<String> {
    p.mappings
        .iter()
        .map(|PathMapping { item, from, to, .. }| {
            format!("{item} {}:{} -> {}:{}", from.0, from.1, to.0, to.1)
        })
        .collect()
}

fn warned(p: &Plan, item: &str, needle: &str) -> bool {
    p.warnings
        .iter()
        .any(|w| w.item.as_ref().is_some_and(|i| i == item) && w.message.contains(needle))
}

#[test]
fn cross_os_item_lands_under_the_macos_slot() {
    let e = env();
    let m = linux_capture(&e, &[]);
    let recv = e.recv.clone().with_os(OsKind::MacOs);
    let p = plan_on(&recv, &[editor()], &m, &only(&[SETTINGS]));
    assert!(p.blockers.is_empty(), "{:?}", p.blockers);
    let w = written(&p, SETTINGS);
    for t in [
        "user_data:Editor/User",
        "user_data:Editor/User/settings.json",
        "user_data:Editor/User/snippets/a.json",
        "home:.editor/ext.txt",
    ] {
        assert!(w.contains(t), "{t} missing from {w:?}");
    }
    assert!(!w.iter().any(|t| t.starts_with("user_config:")), "{w:?}");
    assert_eq!(
        mappings(&p),
        [
            "editor/settings user_config:Editor/User -> user_data:Editor/User",
            "editor/settings home:.editor -> home:.editor",
        ]
    );
    assert!(p.sensitive.is_empty(), "{:?}", p.sensitive);
}

#[test]
fn each_mapping_names_where_its_slot_is_on_this_computer() {
    let e = env();
    let m = linux_capture(&e, &[]);
    let recv = e.recv.clone().with_os(OsKind::MacOs);
    let p = plan_on(&recv, &[editor()], &m, &only(&[SETTINGS]));
    let slot_dir = |root: SemanticRoot, rel: &str| {
        p.actions
            .iter()
            .find(|a| a.root == root && a.rel == rel)
            .map(|a| a.abs.clone())
    };
    let first = &p.mappings[0];
    assert_eq!(first.to, (SemanticRoot::UserData, "Editor/User".to_owned()));
    assert_eq!(
        first.to_abs,
        slot_dir(SemanticRoot::UserData, "Editor/User")
    );
    assert!(first.to_abs.is_some());
}

#[test]
fn cross_os_item_lands_under_the_windows_slot_without_modes() {
    let e = env();
    let m = linux_capture(&e, &[]);
    let recv = e.recv.clone().with_os(OsKind::Windows);
    let p = plan_on(&recv, &[editor()], &m, &only(&[SETTINGS]));
    assert!(p.blockers.is_empty(), "{:?}", p.blockers);
    let w = written(&p, SETTINGS);
    assert!(
        w.contains("user_config:EditorWin/User/snippets/a.json"),
        "{w:?}"
    );
    assert!(!w.iter().any(|t| t.contains("Editor/User")), "{w:?}");
    // Acl: no mode bits are planned (so none are set or reported dropped).
    assert!(
        p.actions.iter().all(|a| a.mode.is_none()),
        "{:#?}",
        p.actions
    );
    assert!(!p.warnings.iter().any(|w| w.message.contains("dropped")));
}

#[test]
fn same_os_item_is_blocked_then_allowed_with_a_warning() {
    let e = env();
    let m = linux_capture(&e, &[]);
    let recv = e.recv.clone().with_os(OsKind::MacOs);
    let p = plan_on(&recv, &[editor()], &m, &only(&[STATE]));
    assert_eq!(
        p.blockers,
        [Blocker::CrossOs {
            item: STATE.into(),
            from: OsKind::Linux,
            to: OsKind::MacOs,
        }]
    );

    let allowed = PlanOptions {
        allow_cross_os: vec![STATE.into()],
        ..only(&[STATE])
    };
    let p = plan_on(&recv, &[editor()], &m, &allowed);
    assert!(p.blockers.is_empty(), "{:?}", p.blockers);
    assert!(warned(&p, STATE, "override"), "{:?}", p.warnings);
    // Restored to the same semantic paths, unmapped.
    assert!(written(&p, STATE).contains("user_config:Editor/state/db"));
    assert!(p.mappings.is_empty(), "{:?}", p.mappings);
}

#[test]
fn same_os_restore_needs_no_override() {
    let e = env();
    let m = linux_capture(&e, &[]);
    let p = plan_on(&e.recv, &[editor()], &m, &only(&[SETTINGS, STATE]));
    assert!(p.blockers.is_empty(), "{:?}", p.blockers);
    assert!(p.mappings.is_empty(), "{:?}", p.mappings);
    assert!(written(&p, SETTINGS).contains("user_config:Editor/User/settings.json"));
}

#[test]
fn cross_os_item_without_a_local_recipe_is_skipped_with_a_warning() {
    let e = env();
    let m = linux_capture(&e, &[]);
    let recv = e.recv.clone().with_os(OsKind::Windows);
    let o = PlanOptions {
        confirmed_sensitive: vec![SETTINGS.into()],
        ..only(&[SETTINGS])
    };
    let p = plan_on(&recv, &[], &m, &o);
    assert!(
        p.actions.iter().all(|a| a.item != SETTINGS),
        "{:#?}",
        p.actions
    );
    assert!(warned(&p, SETTINGS, "no recipe"), "{:?}", p.warnings);
}

#[test]
fn windows_invalid_names_and_symlinks_are_skipped() {
    // The sender's names (reserved, case twins) need a Linux filesystem.
    if carrova_platform::current().os() != OsKind::Linux {
        return;
    }
    let e = env();
    seed(
        &e,
        &[
            ".config/Editor/User/CON.json",
            ".config/Editor/User/com0.log",
            ".config/Editor/User/LPT0",
            ".config/Editor/User/conin$",
            ".config/Editor/User/CONOUT$.txt",
            ".config/Editor/User/COM\u{b9}.json",
            ".config/Editor/User/lpt\u{b3}",
            ".config/Editor/User/NUL .txt",
            ".config/Editor/User/notes.",
            ".config/Editor/User/a<b",
            ".config/Editor/User/back\\slash",
            ".config/Editor/User/Case.json",
            ".config/Editor/User/case.json",
        ],
    );
    symlink(
        "settings.json",
        e.sender_home.join(".config/Editor/User/link"),
    )
    .unwrap();
    let m = capture_linux(&e);
    let recv = e.recv.clone().with_os(OsKind::Windows);
    let p = plan_on(&recv, &[editor()], &m, &only(&[SETTINGS]));
    let base = "EditorWin/User";
    for (name, why) in [
        ("CON.json", "reserved"),
        ("com0.log", "reserved"),
        ("LPT0", "reserved"),
        ("conin$", "reserved"),
        ("CONOUT$.txt", "reserved"),
        ("COM\u{b9}.json", "reserved"),
        ("lpt\u{b3}", "reserved"),
        ("NUL .txt", "reserved"),
        ("notes.", "ends with"),
        ("a<b", "not allowed"),
        ("back\\slash", "not allowed"),
        ("link", "symlink"),
    ] {
        let a = action_of(&p, &format!("{base}/{name}"));
        assert!(is_skip(a, why), "{name}: {a:?}");
    }
    let cased = [
        action_of(&p, &format!("{base}/Case.json")),
        action_of(&p, &format!("{base}/case.json")),
    ];
    assert_eq!(
        cased
            .iter()
            .filter(|a| is_skip(a, "already writes"))
            .count(),
        1,
        "{cased:?}"
    );
}

#[test]
fn user_remap_wins_over_the_cross_os_mapping() {
    let e = env();
    let m = linux_capture(&e, &[]);
    let recv = e.recv.clone().with_os(OsKind::MacOs);
    let o = PlanOptions {
        remaps: [(
            SETTINGS.into(),
            Remap {
                root: Some(SemanticRoot::Documents),
                rel_prefix: Some("restored".into()),
            },
        )]
        .into(),
        ..only(&[SETTINGS])
    };
    let p = plan_on(&recv, &[editor()], &m, &o);
    let w = written(&p, SETTINGS);
    assert!(
        w.contains("documents:restored/Editor/User/settings.json"),
        "{w:?}"
    );
    assert!(w.iter().all(|t| t.starts_with("documents:")), "{w:?}");
    assert!(p.mappings.is_empty(), "{:?}", p.mappings);
}

#[test]
fn allowing_an_unknown_item_cross_os_is_refused() {
    let e = env();
    let m = linux_capture(&e, &[]);
    let o = PlanOptions {
        allow_cross_os: vec!["nope/x".into()],
        ..PlanOptions::default()
    };
    let err = plan_on_date(&e.recv, &[editor()], &m, &o, &sid(), DATE).unwrap_err();
    assert!(err.to_string().contains("allow_cross_os"), "{err}");
}
