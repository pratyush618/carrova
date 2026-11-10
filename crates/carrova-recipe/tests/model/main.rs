//! Per-OS views of a recipe: `Item::paths_for`, `Item::exclude_for`,
//! `Recipe::supports`.

#![allow(clippy::unwrap_used)]

use std::path::Path;

use carrova_platform::OsKind;
use carrova_recipe::{Recipe, load_str};

const RECIPE: &str = r#"
schema: 3
id: demo
name: Demo
category: dev-tools
detect:
  linux: [{ binary: demo }]
  windows: [{ binary: demo }]
items:
  - id: config
    label: Config
    paths: [{ root: user_config, rel: demo }]
    exclude: ["**/cache"]
    os:
      macos:
        paths: [{ root: user_data, rel: Demo }]
      windows:
        paths: [{ root: user_config, rel: Demo }]
        exclude: []
"#;

fn recipe() -> Recipe {
    load_str(RECIPE, Path::new("t.yaml")).unwrap()
}

fn rels(r: &Recipe, os: OsKind) -> Vec<String> {
    r.items[0]
        .paths_for(os)
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[test]
fn paths_for_uses_the_os_override_else_the_base_paths() {
    let r = recipe();
    assert_eq!(rels(&r, OsKind::Linux), ["user_config:demo"]);
    assert_eq!(rels(&r, OsKind::MacOs), ["user_data:Demo"]);
    assert_eq!(rels(&r, OsKind::Windows), ["user_config:Demo"]);
}

#[test]
fn exclude_for_inherits_unless_the_override_lists_its_own() {
    let r = recipe();
    let item = &r.items[0];
    assert_eq!(item.exclude_for(OsKind::Linux), ["**/cache"]);
    assert_eq!(
        item.exclude_for(OsKind::MacOs),
        ["**/cache"],
        "absent = inherit"
    );
    assert!(item.exclude_for(OsKind::Windows).is_empty(), "[] = none");
}

#[test]
fn item_without_overrides_uses_base_paths_everywhere() {
    let mut r = recipe();
    r.items[0].os = None;
    for os in OsKind::ALL {
        assert_eq!(rels(&r, os), ["user_config:demo"], "{os}");
    }
}

#[test]
fn recipe_supports_exactly_the_oses_with_detect_rules() {
    let r = recipe();
    assert!(r.supports(OsKind::Linux));
    assert!(r.supports(OsKind::Windows));
    assert!(!r.supports(OsKind::MacOs));
}

#[test]
fn overrides_roundtrip_through_yaml() {
    let r = recipe();
    let text = serde_yaml_ng::to_string(&r).unwrap();
    assert_eq!(load_str(&text, Path::new("x")).unwrap(), r);
}
