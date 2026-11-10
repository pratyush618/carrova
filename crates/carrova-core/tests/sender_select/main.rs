//! Sender selection: presets, suggestions, confirmations, exclusions.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;

use carrova_core::select::{Preset, Selection, SelectionEdit};
use carrova_core::{ItemKey, Recipe, ScanOptions, scan};
use carrova_platform::fake::FakePlatform;
use carrova_recipe::load_str;
use tempfile::TempDir;

const APP: &str = r"
schema: 1
id: app
name: App
category: dev
detect:
  linux:
    - binary: app
items:
  - id: config
    label: Config
    paths:
      - { root: user_config, rel: app }
  - id: dotfile
    label: Dotfile
    paths:
      - { root: home, rel: .apprc }
  - id: data
    label: Data
    paths:
      - { root: user_data, rel: app }
  - id: cache
    label: Cache
    paths:
      - { root: cache, rel: app }
  - id: optional
    label: Optional
    paths:
      - { root: user_config, rel: app-extra }
    flags: { default: false }
  - id: key
    label: Key
    paths:
      - { root: home, rel: .app-key }
    flags: { sensitive: true }
  - id: locked
    label: Locked
    paths:
      - { root: user_data, rel: app-locked }
    unmovable: machine-bound
";

fn setup() -> (TempDir, FakePlatform, Vec<Recipe>) {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    for d in [
        ".config/app",
        ".config/app-extra",
        ".config/stray",
        ".local/share/app",
        ".local/share/app-locked",
        ".cache/app",
    ] {
        fs::create_dir_all(home.join(d)).unwrap();
        fs::write(home.join(d).join("f"), b"x").unwrap();
    }
    fs::write(home.join(".apprc"), b"rc").unwrap();
    fs::write(home.join(".app-key"), b"secret").unwrap();
    let platform = FakePlatform::with_home(home).binary("app", "/usr/bin/app");
    let recipes = vec![load_str(APP, Path::new("app.yaml")).unwrap()];
    (tmp, platform, recipes)
}

fn selection(preset: Preset) -> Selection {
    let (_tmp, p, recipes) = setup();
    let report = scan(&recipes, &p, ScanOptions::default()).unwrap();
    Selection::from_scan(&report, preset)
}

fn included(sel: &Selection) -> Vec<&str> {
    sel.included().map(ItemKey::as_str).collect()
}

#[test]
fn suggestions_skip_sensitive_raw_cache_unmovable() {
    let sel = selection(Preset::None);
    let suggested: Vec<&str> = sel
        .items
        .iter()
        .filter(|(_, c)| c.suggested)
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(suggested, ["app/config", "app/data", "app/dotfile"]);
    assert!(sel.items.contains_key("raw:user_config/stray"));
    assert!(sel.items["app/key"].sensitive);
    assert_eq!(
        sel.items["app/locked"].unmovable.as_deref(),
        Some("machine-bound")
    );
}

#[test]
fn presets_fill_included() {
    assert_eq!(
        included(&selection(Preset::Suggested)),
        ["app/config", "app/data", "app/dotfile"]
    );
    assert_eq!(
        included(&selection(Preset::Minimal)),
        ["app/config", "app/dotfile"]
    );
    assert_eq!(
        included(&selection(Preset::Everything)),
        [
            "app/cache",
            "app/config",
            "app/data",
            "app/dotfile",
            "app/optional",
            "raw:user_config/stray",
        ]
    );
    assert!(included(&selection(Preset::None)).is_empty());
}

#[test]
fn apply_preset_keeps_confirmations_and_exclusions() {
    let mut sel = selection(Preset::Suggested);
    sel.confirm_sensitive("app/key").unwrap();
    sel.set_path_excluded("app/config", "user_config:app/f", true)
        .unwrap();
    sel.apply_preset(Preset::None);
    assert!(included(&sel).is_empty());
    assert!(sel.items["app/key"].confirmed_sensitive);
    assert_eq!(
        sel.items["app/config"].excluded_paths,
        ["user_config:app/f"]
    );
}

#[test]
fn sensitive_needs_confirmation() {
    let mut sel = selection(Preset::None);
    sel.set_included("app/key", true).unwrap();
    let issues = sel.issues();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].item, "app/key");
    sel.confirm_sensitive("app/key").unwrap();
    assert!(sel.issues().is_empty());
}

#[test]
fn unmovable_needs_force() {
    let mut sel = selection(Preset::None);
    sel.set_included("app/locked", true).unwrap();
    let issues = sel.issues();
    assert_eq!(issues.len(), 1);
    assert!(issues[0].problem.contains("machine-bound"));
    sel.force_unmovable("app/locked").unwrap();
    assert!(sel.issues().is_empty());
}

#[test]
fn excluded_path_must_be_under_item() {
    let mut sel = selection(Preset::None);
    sel.set_path_excluded("app/config", "user_config:app/f", true)
        .unwrap();
    assert!(sel.issues().is_empty());
    sel.set_path_excluded("app/config", "user_config:other", true)
        .unwrap();
    sel.set_path_excluded("app/config", "garbage", true)
        .unwrap();
    assert_eq!(sel.issues().len(), 2);
    sel.set_path_excluded("app/config", "user_config:other", false)
        .unwrap();
    sel.set_path_excluded("app/config", "garbage", false)
        .unwrap();
    assert!(sel.issues().is_empty());
    // `app` is not under `app-extra` despite the shared prefix.
    sel.set_path_excluded("app/optional", "user_config:app", true)
        .unwrap();
    assert_eq!(sel.issues().len(), 1);
}

#[test]
fn unknown_keys_are_errors() {
    let mut sel = selection(Preset::None);
    assert!(sel.set_included("nope/x", true).is_err());
    assert!(sel.set_path_excluded("nope/x", "home:a", true).is_err());
    assert!(sel.confirm_sensitive("nope/x").is_err());
    assert!(sel.force_unmovable("nope/x").is_err());
}

#[test]
fn item_paths_come_from_scan_report() {
    let (_tmp, p, recipes) = setup();
    let report = scan(&recipes, &p, ScanOptions::default()).unwrap();
    let sel = Selection::from_scan(&report, Preset::Suggested);
    assert_eq!(sel.items.len(), 8);
    assert!(!sel.items["app/config"].paths.is_empty());
    assert_eq!(
        sel.items["raw:user_config/stray"].paths,
        ["user_config:stray"]
    );
}

#[test]
fn edits_are_plain_changes_to_the_selection() {
    let mut sel = selection(Preset::None);
    let edits = [
        SelectionEdit::Item {
            item: "app/key".into(),
            included: true,
        },
        SelectionEdit::ConfirmSensitive {
            item: "app/key".into(),
            confirmed: true,
        },
        SelectionEdit::Path {
            item: "app/config".into(),
            path: "user_config:app/f".into(),
            included: false,
        },
        SelectionEdit::ForceUnmovable {
            item: "app/locked".into(),
        },
    ];
    for edit in edits {
        sel.apply(edit).unwrap();
    }
    assert_eq!(included(&sel), ["app/key"]);
    assert!(sel.items["app/key"].confirmed_sensitive);
    assert!(sel.items["app/locked"].force_unmovable);
    assert_eq!(
        sel.items["app/config"].excluded_paths,
        ["user_config:app/f"]
    );
    sel.apply(SelectionEdit::Path {
        item: "app/config".into(),
        path: "user_config:app/f".into(),
        included: true,
    })
    .unwrap();
    assert!(sel.items["app/config"].excluded_paths.is_empty());
    assert!(sel.issues().is_empty());
}

#[test]
fn an_app_row_ticks_only_its_ordinary_items() {
    let mut sel = selection(Preset::None);
    sel.apply(SelectionEdit::App {
        app: "app".into(),
        included: true,
    })
    .unwrap();
    assert_eq!(
        included(&sel),
        [
            "app/cache",
            "app/config",
            "app/data",
            "app/dotfile",
            "app/optional"
        ]
    );
    sel.apply(SelectionEdit::App {
        app: "app".into(),
        included: false,
    })
    .unwrap();
    assert!(included(&sel).is_empty());
}

#[test]
fn the_caches_switch_and_presets_are_edits_too() {
    let mut sel = selection(Preset::Suggested);
    sel.apply(SelectionEdit::Caches { included: true }).unwrap();
    assert_eq!(
        included(&sel),
        ["app/cache", "app/config", "app/data", "app/dotfile"]
    );
    sel.apply(SelectionEdit::Preset {
        preset: Preset::None,
    })
    .unwrap();
    assert!(included(&sel).is_empty());
}

#[test]
fn an_edit_naming_an_unknown_item_is_an_issue_and_changes_nothing() {
    let mut sel = selection(Preset::Suggested);
    let before = sel.clone();
    for edit in [
        SelectionEdit::Item {
            item: "nope/x".into(),
            included: true,
        },
        SelectionEdit::Path {
            item: "nope/x".into(),
            path: "home:a".into(),
            included: false,
        },
        SelectionEdit::ConfirmSensitive {
            item: "nope/x".into(),
            confirmed: true,
        },
        SelectionEdit::ForceUnmovable {
            item: "nope/x".into(),
        },
    ] {
        let issue = sel.apply(edit).unwrap_err();
        assert_eq!(issue.item, "nope/x");
    }
    assert_eq!(sel, before);
}
