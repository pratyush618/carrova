//! End-to-end tests of the `carrova` binary.
//!
//! `scan` runs against the real platform adapter with HOME, XDG_* and PATH
//! pointed into a temp dir. Phase 1 targets Linux only; on other OSes those
//! tests return early at runtime.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use carrova_platform::OsKind;
use tempfile::TempDir;

const GOOD: &str = r"
schema: 1
id: demo
name: Demo
category: dev-tools
detect:
  linux:
    - binary: demo
items:
  - id: config
    label: Demo config
    paths:
      - { root: user_config, rel: demo }
  - id: token
    label: Demo token
    paths:
      - { root: home, rel: .demo-token }
    flags: { sensitive: true }
";

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A recipe tree with one valid recipe.
fn recipe_tree() -> TempDir {
    let tmp = TempDir::new().unwrap();
    write(&tmp.path().join("dev-tools/demo/recipe.yaml"), GOOD);
    tmp
}

fn carrova(args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_carrova"));
    cmd.args(args)
        .env_remove("CARROVA_RECIPES")
        .env_remove("RUST_LOG");
    cmd
}

fn text(out: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn validate_good_tree_succeeds() {
    let tree = recipe_tree();
    let out = carrova(&[
        "--recipes",
        tree.path().to_str().unwrap(),
        "recipes",
        "validate",
    ])
    .output()
    .unwrap();
    let (stdout, stderr) = text(&out);
    assert!(out.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains("1 recipes, 0 issues"), "{stdout}");
}

#[test]
fn validate_bad_recipe_fails_with_issue() {
    let tree = recipe_tree();
    // Category does not match its directory; item has no paths.
    write(
        &tree.path().join("shells/bad/recipe.yaml"),
        "schema: 1\nid: bad\nname: Bad\ncategory: wrong\ndetect:\n  linux:\n    - binary: bad\nitems:\n  - id: x\n    label: X\n",
    );
    let out = carrova(&["recipes", "validate", tree.path().to_str().unwrap()])
        .output()
        .unwrap();
    let (stdout, _) = text(&out);
    assert!(!out.status.success());
    assert!(
        stdout.contains("category `wrong` does not match directory `shells`"),
        "{stdout}"
    );
    assert!(stdout.contains("items[0]"), "{stdout}");
    assert!(stdout.contains("2 recipes, 2 issues"), "{stdout}");
}

#[test]
fn validate_single_recipe_dir_and_file() {
    let tree = recipe_tree();
    let dir = tree.path().join("dev-tools/demo");
    for target in [dir.clone(), dir.join("recipe.yaml")] {
        let out = carrova(&["recipes", "validate", target.to_str().unwrap()])
            .output()
            .unwrap();
        let (stdout, _) = text(&out);
        assert!(out.status.success(), "{stdout}");
        assert!(stdout.contains("1 recipes, 0 issues"), "{stdout}");
    }
}

#[test]
fn validate_unparseable_recipe_fails() {
    let tree = recipe_tree();
    write(&tree.path().join("shells/broken/recipe.yaml"), "id: [oops");
    let out = carrova(&[
        "--recipes",
        tree.path().to_str().unwrap(),
        "recipes",
        "validate",
    ])
    .output()
    .unwrap();
    let (stdout, _) = text(&out);
    assert!(!out.status.success());
    assert!(stdout.contains("broken"), "{stdout}");
    assert!(stdout.contains("1 load errors"), "{stdout}");
}

#[test]
fn list_shows_recipes() {
    let tree = recipe_tree();
    let out = carrova(&[
        "--recipes",
        tree.path().to_str().unwrap(),
        "recipes",
        "list",
    ])
    .output()
    .unwrap();
    let (stdout, _) = text(&out);
    assert!(out.status.success());
    assert!(stdout.contains("demo  Demo  dev-tools"), "{stdout}");
}

/// Fake home + PATH with a `demo` executable. Returns `(recipe tree, home tmp, command)`;
/// both temp dirs must outlive the command.
fn scan_cmd(args: &[&str]) -> Option<(TempDir, TempDir, Command)> {
    if carrova_platform::current().os() != OsKind::Linux {
        return None;
    }
    let tree = recipe_tree();
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let bin = tmp.path().join("bin");
    write(&home.join(".config/demo/settings.json"), "{\"a\": 1}\n");
    write(&home.join(".config/mystery/data.bin"), "0123456789");
    write(&home.join(".local/share/.keep"), "");
    let exe = bin.join("demo");
    write(&exe, "#!/bin/sh\n");
    // Borrow the exec permission bits of the carrova binary itself (portable).
    let perms = fs::metadata(env!("CARGO_BIN_EXE_carrova"))
        .unwrap()
        .permissions();
    fs::set_permissions(&exe, perms).unwrap();

    let mut full = vec!["--recipes", tree.path().to_str().unwrap()];
    full.extend_from_slice(args);
    let mut cmd = carrova(&full);
    cmd.env("HOME", &home)
        .env("PATH", &bin)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CACHE_HOME");
    Some((tree, tmp, cmd))
}

#[test]
fn scan_json_reports_detected_app() {
    let Some((_tree, _tmp, mut cmd)) = scan_cmd(&["scan", "--json"]) else {
        return;
    };
    let out = cmd.output().unwrap();
    let (stdout, stderr) = text(&out);
    assert!(out.status.success(), "{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is pure JSON");
    let apps = v["apps"].as_array().unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0]["id"], "demo");
    assert_eq!(apps[0]["detected"], true);
    let config = &apps[0]["items"][0];
    assert_eq!(config["size"], 9);
    assert_eq!(config["file_count"], 1);
    assert_eq!(apps[0]["items"][1]["flags"]["default_selected"], false);
    assert_eq!(v["unrecognised"][0]["rel"], "mystery");
    assert_eq!(v["unrecognised"][0]["size"], 10);
}

#[test]
fn scan_human_output() {
    let Some((_tree, _tmp, mut cmd)) = scan_cmd(&["scan"]) else {
        return;
    };
    let out = cmd.output().unwrap();
    let (stdout, stderr) = text(&out);
    assert!(out.status.success(), "{stderr}");
    assert!(stdout.contains("dev-tools / Demo"), "{stdout}");
    assert!(
        stdout.contains("[sensitive]  [off]  [not found]"),
        "{stdout}"
    );
    assert!(stdout.contains("Unrecognised data"), "{stdout}");
    assert!(stdout.contains("user_config/mystery"), "{stdout}");
    assert!(stdout.contains("selected by default"), "{stdout}");
}

#[test]
fn scan_all_lists_undetected() {
    let Some((tree, _tmp, mut cmd)) = scan_cmd(&["scan", "--json", "--all"]) else {
        return;
    };
    write(
        &tree.path().join("shells/absent/recipe.yaml"),
        "schema: 1\nid: absent\nname: Absent\ncategory: shells\ndetect:\n  linux:\n    - binary: absent\nitems:\n  - id: x\n    label: X\n    paths:\n      - { root: home, rel: .absent }\n",
    );
    let out = cmd.output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<(&str, bool)> = v["apps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["id"].as_str().unwrap(), a["detected"].as_bool().unwrap()))
        .collect();
    assert_eq!(ids, [("demo", true), ("absent", false)]);
}
