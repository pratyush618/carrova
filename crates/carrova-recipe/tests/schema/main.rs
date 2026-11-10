//! Keeps `recipes/schema/recipe.v{1,2}.json` and the serde types + validator
//! in sync. Documents are checked against the schema file named by their
//! `schema` field; any other value fails JSON validation.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use carrova_recipe::{Handler, SUPPORTED_SCHEMA, load_str, validate};

fn schema_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes/schema")
}

fn schema_path(version: u32) -> PathBuf {
    schema_dir().join(format!("recipe.v{version}.json"))
}

fn schema_json(version: u32) -> serde_json::Value {
    let text = std::fs::read_to_string(schema_path(version)).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// One compiled validator per supported schema version.
struct Schemas(Vec<(u32, jsonschema::Validator)>);

impl Schemas {
    fn load() -> Self {
        Self(
            SUPPORTED_SCHEMA
                .map(|v| (v, jsonschema::validator_for(&schema_json(v)).unwrap()))
                .collect(),
        )
    }

    /// Validator picked by the document's `schema` field.
    fn for_doc(&self, json: &serde_json::Value) -> Option<&jsonschema::Validator> {
        let v = json.get("schema")?.as_u64()?;
        self.0
            .iter()
            .find(|(n, _)| u64::from(*n) == v)
            .map(|(_, val)| val)
    }

    fn errors(&self, json: &serde_json::Value) -> Vec<String> {
        match self.for_doc(json) {
            Some(val) => val.iter_errors(json).map(|e| e.to_string()).collect(),
            None => vec!["no schema file for this `schema` value".into()],
        }
    }
}

const BASE: &str = r#"
schema: 1
id: git
name: Git
category: dev-tools
description: Distributed VCS
detect:
  linux:
    - binary: git
    - path: { root: home, rel: .gitconfig }
items:
  - id: config
    label: Global git config
    paths:
      - { root: home, rel: .gitconfig }
      - { root: user_config, rel: git }
    exclude: ["**/credentials"]
    flags:
      portability: same_os
      sensitive: false
      requires_closed: false
      default: true
"#;

fn with_item(item: &str) -> String {
    format!("{BASE}{item}")
}

/// (name, yaml, expected valid)
fn cases() -> Vec<(&'static str, String, bool)> {
    let r = |from: &str, to: &str| BASE.replacen(from, to, 1);
    vec![
        ("base", BASE.to_owned(), true),
        (
            "no description",
            r("description: Distributed VCS\n", ""),
            true,
        ),
        (
            "flags omitted",
            r(
                "    flags:\n      portability: same_os\n      sensitive: false\n      requires_closed: false\n      default: true\n",
                "",
            ),
            true,
        ),
        ("cross_os", r("same_os", "cross_os"), true),
        (
            "macos detect",
            r("items:", "  macos: [{ binary: git }]\nitems:"),
            true,
        ),
        ("dot-slash rel", r("rel: git }", "rel: ./git/ }"), true),
        (
            "sensitive default omitted",
            with_item(
                "  - id: creds\n    label: Creds\n    paths: [{ root: home, rel: .git-credentials }]\n    flags: { sensitive: true }\n",
            ),
            true,
        ),
        (
            "sensitive default false",
            with_item(
                "  - id: creds\n    label: Creds\n    paths: [{ root: home, rel: .git-credentials }]\n    flags: { sensitive: true, default: false }\n",
            ),
            true,
        ),
        (
            "unmovable no paths",
            with_item("  - id: kr\n    label: Keyring\n    unmovable: OS-bound\n"),
            true,
        ),
        (
            "unmovable default false",
            with_item(
                "  - id: kr\n    label: Keyring\n    unmovable: OS-bound\n    flags: { default: false }\n",
            ),
            true,
        ),
        // ---- invalid ----
        ("schema 3", r("schema: 1", "schema: 3"), false),
        ("schema 0", r("schema: 1", "schema: 0"), false),
        ("schema missing", r("schema: 1\n", ""), false),
        (
            "unknown top field",
            r("name: Git", "name: Git\nhomepage: x"),
            false,
        ),
        ("bad id", r("id: git", "id: Git_Tool"), false),
        (
            "bad category",
            r("category: dev-tools", "category: dev--tools"),
            false,
        ),
        ("empty name", r("name: Git", "name: ''"), false),
        (
            "no linux detect",
            r(
                "  linux:\n    - binary: git\n    - path: { root: home, rel: .gitconfig }\n",
                "  macos: [{ binary: git }]\n",
            ),
            false,
        ),
        (
            "empty linux detect",
            r(
                "  linux:\n    - binary: git\n    - path: { root: home, rel: .gitconfig }\n",
                "  linux: []\n",
            ),
            false,
        ),
        (
            "unknown detect os",
            r("items:", "  bsd: [{ binary: git }]\nitems:"),
            false,
        ),
        (
            "detect both keys",
            r(
                "- binary: git",
                "- { binary: git, path: { root: home, rel: x } }",
            ),
            false,
        ),
        ("detect empty rule", r("- binary: git", "- {}"), false),
        (
            "binary with slash",
            r("- binary: git", "- binary: /usr/bin/git"),
            false,
        ),
        ("bad root", r("root: user_config", "root: etc"), false),
        ("rel parent", r("rel: git }", "rel: ../git }"), false),
        (
            "rel inner parent",
            r("rel: git }", "rel: a/../git }"),
            false,
        ),
        ("rel absolute", r("rel: git }", "rel: /etc/git }"), false),
        ("rel tilde", r("rel: git }", "rel: ~/git }"), false),
        ("rel backslash", r("rel: git }", "rel: 'a\\b' }"), false),
        ("rel drive", r("rel: git }", "rel: 'C:/x' }"), false),
        ("rel empty", r("rel: git }", "rel: '' }"), false),
        ("rel dot", r("rel: git }", "rel: ./ }"), false),
        (
            "unknown path key",
            r("rel: git }", "rel: git, mode: 1 }"),
            false,
        ),
        (
            "no items",
            r(&BASE[BASE.find("items:").unwrap_or(0)..], "items: []\n"),
            false,
        ),
        ("bad item id", r("- id: config", "- id: Config"), false),
        (
            "empty label",
            r("label: Global git config", "label: ''"),
            false,
        ),
        (
            "unknown item field",
            r(
                "label: Global git config",
                "label: G\n    export: git config -l",
            ),
            false,
        ),
        (
            "unknown flag",
            r("sensitive: false", "secret: false"),
            false,
        ),
        ("bad portability", r("same_os", "any_os"), false),
        (
            "non-bool flag",
            r("sensitive: false", "sensitive: maybe"),
            false,
        ),
        ("empty exclude", r("[\"**/credentials\"]", "['']"), false),
        (
            "paths missing",
            with_item("  - id: x\n    label: X\n"),
            false,
        ),
        (
            "paths empty",
            with_item("  - id: x\n    label: X\n    paths: []\n"),
            false,
        ),
        (
            "sensitive default true",
            with_item(
                "  - id: creds\n    label: Creds\n    paths: [{ root: home, rel: .git-credentials }]\n    flags: { sensitive: true, default: true }\n",
            ),
            false,
        ),
        (
            "unmovable default true",
            with_item(
                "  - id: kr\n    label: Keyring\n    unmovable: OS-bound\n    flags: { default: true }\n",
            ),
            false,
        ),
        (
            "unmovable empty reason",
            with_item("  - id: kr\n    label: Keyring\n    unmovable: ''\n"),
            false,
        ),
        // v2-only fields are rejected in v1.
        (
            "v1 processes",
            r("items:", "processes: [git-agent]\nitems:"),
            false,
        ),
        (
            "v1 handler",
            r(
                "label: Global git config",
                "label: Global git config\n    handler: gpg-keyring",
            ),
            false,
        ),
    ]
}

/// v2: every v1 case except those about the version itself, with
/// `schema: 2`, plus the v2-only fields.
fn cases_v2() -> Vec<(String, String, bool)> {
    let v2 = BASE.replacen("schema: 1", "schema: 2", 1);
    let r = |from: &str, to: &str| v2.replacen(from, to, 1);
    let handler = |extra: &str| {
        r(
            "label: Global git config",
            &format!("label: Global git config\n    handler: gpg-keyring{extra}"),
        )
    };
    let mut out: Vec<(String, String, bool)> = cases()
        .into_iter()
        .filter(|(name, _, _)| !name.starts_with("schema ") && !name.starts_with("v1 "))
        .map(|(name, yaml, want)| {
            (
                format!("v2 {name}"),
                yaml.replacen("schema: 1", "schema: 2", 1),
                want,
            )
        })
        .collect();
    let extra = [
        ("schema 2", v2.clone(), true),
        (
            "processes",
            r("items:", "processes: [gpg-agent, scdaemon]\nitems:"),
            true,
        ),
        ("processes empty list", r("items:", "processes: []\nitems:"), true),
        ("handler", handler(""), true),
        (
            "handler sensitive",
            with_item(
                "  - id: keys\n    label: Keys\n    handler: gpg-keyring\n    paths: [{ root: home, rel: .gnupg/pubring.kbx }]\n    flags: { sensitive: true }\n",
            )
            .replacen("schema: 1", "schema: 2", 1),
            true,
        ),
        (
            "process with slash",
            r("items:", "processes: [/usr/bin/gpg-agent]\nitems:"),
            false,
        ),
        ("process empty", r("items:", "processes: ['']\nitems:"), false),
        (
            "process not a list",
            r("items:", "processes: gpg-agent\nitems:"),
            false,
        ),
        ("unknown handler", r("label: Global git config", "label: G\n    handler: gpg-export"), false),
        (
            "handler empty",
            r("label: Global git config", "label: G\n    handler: ''"),
            false,
        ),
        (
            "handler without paths",
            with_item("  - id: kr\n    label: Keyring\n    handler: gpg-keyring\n")
                .replacen("schema: 1", "schema: 2", 1),
            false,
        ),
        (
            "handler on unmovable",
            with_item(
                "  - id: kr\n    label: Keyring\n    unmovable: OS-bound\n    handler: gpg-keyring\n",
            )
            .replacen("schema: 1", "schema: 2", 1),
            false,
        ),
    ];
    out.extend(extra.into_iter().map(|(n, y, w)| (format!("v2 {n}"), y, w)));
    out
}

#[test]
fn schema_matches_serde() {
    let schemas = Schemas::load();
    let all = cases()
        .into_iter()
        .map(|(n, y, w)| (n.to_owned(), y, w))
        .chain(cases_v2());

    let mut mismatches = Vec::new();
    for (name, yaml, want) in all {
        let json: serde_json::Value = serde_yaml_ng::from_str(&yaml).unwrap();
        let errs = schemas.errors(&json);
        let by_schema = errs.is_empty();
        let by_rust = load_str(&yaml, Path::new(&name)).is_ok_and(|r| validate(&r).is_empty());
        if by_schema != want || by_rust != want {
            mismatches.push(format!(
                "{name}: want {want}, schema {by_schema} {errs:?}, rust {by_rust}"
            ));
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// The schema's handler enum lists exactly [`Handler::ALL`].
#[test]
fn handler_enum_matches_schema() {
    let schema = schema_json(2);
    let names: Vec<&str> = schema["$defs"]["handler"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let rust: Vec<&str> = Handler::ALL.iter().map(|h| h.as_str()).collect();
    assert_eq!(names, rust);
}

/// One schema file per supported version, none beyond.
#[test]
fn schema_files_match_supported_versions() {
    for v in SUPPORTED_SCHEMA {
        assert!(
            schema_path(v).is_file(),
            "missing {}",
            schema_path(v).display()
        );
    }
    assert!(!schema_path(SUPPORTED_SCHEMA.end() + 1).exists());
}

/// Checks the schema cannot express; only the Rust validator catches them.
#[test]
fn validator_only_checks() {
    let schemas = Schemas::load();
    for yaml in [
        with_item("  - id: config\n    label: Dup\n    paths: [{ root: home, rel: x }]\n"),
        BASE.replacen("[\"**/credentials\"]", "[\"[unclosed\"]", 1),
        // sensitive path under a non-sensitive item without a matching exclude
        with_item(
            "  - id: creds\n    label: Creds\n    paths: [{ root: user_config, rel: git/token }]\n    flags: { sensitive: true }\n",
        ),
    ] {
        let v2 = yaml.replacen("schema: 1", "schema: 2", 1);
        for yaml in [yaml, v2] {
            let json: serde_json::Value = serde_yaml_ng::from_str(&yaml).unwrap();
            assert_eq!(schemas.errors(&json), Vec::<String>::new());
            let r = load_str(&yaml, Path::new("t")).unwrap();
            assert_eq!(validate(&r).len(), 1);
        }
    }
}

/// Every shipped `recipes/<category>/<app>/recipe.yaml` passes the JSON Schema
/// for its version.
#[test]
fn shipped_recipes_match_schema() {
    let schemas = Schemas::load();
    let tree = carrova_recipe::load_tree(schema_dir().parent().unwrap()).unwrap();
    assert!(tree.errors.is_empty(), "{:#?}", tree.errors);
    assert!(!tree.recipes.is_empty(), "no recipes found");
    let mut failures = Vec::new();
    for loaded in &tree.recipes {
        let yaml = std::fs::read_to_string(&loaded.path).unwrap();
        let json: serde_json::Value = serde_yaml_ng::from_str(&yaml).unwrap();
        for e in schemas.errors(&json) {
            failures.push(format!("{}: {e}", loaded.path.display()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
