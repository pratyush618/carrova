//! Windows-native `\` separators are accepted only inside `windows:` blocks
//! (`os.windows` item overrides, `detect.windows` rules), by both the loader
//! and `recipe.v3.json`.

#![allow(clippy::unwrap_used)]

use std::path::Path;

use carrova_platform::OsKind;
use carrova_recipe::{DetectRule, load_str, validate};

/// A v3 recipe whose single item has `rel` in the `os.<os>` override.
fn recipe(os: &str, rel: &str) -> String {
    format!(
        r#"
schema: 3
id: demo
name: Demo
category: dev-tools
detect:
  linux: [{{ binary: demo }}]
items:
  - id: config
    label: Config
    paths: [{{ root: user_config, rel: demo }}]
    os:
      {os}:
        paths: [{{ root: user_config, rel: '{rel}' }}]
"#
    )
}

fn schema_accepts(yaml: &str) -> bool {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes/schema/recipe.v3.json");
    let schema = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let doc: serde_json::Value = serde_yaml_ng::from_str(yaml).unwrap();
    jsonschema::validator_for(&schema).unwrap().is_valid(&doc)
}

#[test]
fn windows_block_normalises_backslashes() {
    let yaml = recipe("windows", "AppData\\Code\\User");
    let r = load_str(&yaml, Path::new("t")).unwrap();
    assert!(validate(&r).is_empty(), "{:#?}", validate(&r));
    let paths = r.items[0].paths_for(OsKind::Windows);
    assert_eq!(paths[0].rel.as_str(), "AppData/Code/User");
    assert!(schema_accepts(&yaml));
}

#[test]
fn backslash_rejected_outside_windows_block() {
    for os in ["linux", "macos"] {
        let yaml = recipe(os, "AppData\\Code\\User");
        assert!(load_str(&yaml, Path::new("t")).is_err(), "{os}");
        assert!(!schema_accepts(&yaml), "{os}");
    }
}

#[test]
fn windows_block_rejects_absolute_and_escapes() {
    for rel in [
        "C:\\x",
        "C:/x",
        "\\\\srv\\x",
        "\\x",
        "..\\x",
        "a\\..\\..\\x",
        ".\\",
    ] {
        let yaml = recipe("windows", rel);
        assert!(load_str(&yaml, Path::new("t")).is_err(), "{rel}");
        assert!(!schema_accepts(&yaml), "{rel}");
    }
}

/// A v3 recipe detected only by a `detect.<os>` path rule with `rel`.
fn detect_recipe(os: &str, rel: &str) -> String {
    format!(
        r#"
schema: 3
id: demo
name: Demo
category: dev-tools
detect:
  {os}: [{{ path: {{ root: user_config, rel: '{rel}' }} }}]
items:
  - id: config
    label: Config
    paths: [{{ root: user_config, rel: demo }}]
"#
    )
}

#[test]
fn windows_detect_rule_normalises_backslashes() {
    let yaml = detect_recipe("windows", "AppData\\Code");
    let r = load_str(&yaml, Path::new("t")).unwrap();
    assert!(validate(&r).is_empty(), "{:#?}", validate(&r));
    let [DetectRule::Path(p)] = r.detect.rules(OsKind::Windows) else {
        panic!("expected one path rule: {:?}", r.detect);
    };
    assert_eq!(p.rel.as_str(), "AppData/Code");
    assert!(schema_accepts(&yaml));
}

#[test]
fn detect_backslash_rejected_outside_windows() {
    for os in ["linux", "macos"] {
        let yaml = detect_recipe(os, "AppData\\Code");
        assert!(load_str(&yaml, Path::new("t")).is_err(), "{os}");
        assert!(!schema_accepts(&yaml), "{os}");
    }
}

#[test]
fn windows_detect_rule_rejects_absolute_and_escapes() {
    for rel in ["C:\\x", "\\\\srv\\x", "..\\x"] {
        let yaml = detect_recipe("windows", rel);
        assert!(load_str(&yaml, Path::new("t")).is_err(), "{rel}");
        assert!(!schema_accepts(&yaml), "{rel}");
    }
}
