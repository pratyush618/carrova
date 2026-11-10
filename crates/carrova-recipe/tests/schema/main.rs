//! Keeps `recipes/schema/recipe.v{1,2,3}.json` and the serde types + validator
//! in sync. Documents are checked against the schema file named by their
//! `schema` field; any other value fails JSON validation.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use carrova_recipe::{Handler, SUPPORTED_SCHEMA, load_str, validate};

mod cases;
mod cases_v3;

use cases::{BASE, cases, cases_v2, with_item};
use cases_v3::{cases_v3, validator_only_v3};

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

#[test]
fn schema_matches_serde() {
    let schemas = Schemas::load();
    let all = cases()
        .into_iter()
        .map(|(n, y, w)| (n.to_owned(), y, w))
        .chain(cases_v2())
        .chain(cases_v3());

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

/// Every schema's built-in handler enum lists exactly [`Handler::ALL`].
#[test]
fn handler_enum_matches_schema() {
    let rust: Vec<&str> = Handler::ALL.iter().map(|h| h.as_str()).collect();
    for (version, def) in [(2, "handler"), (3, "builtinHandler")] {
        let schema = schema_json(version);
        let names: Vec<&str> = schema["$defs"][def]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(names, rust, "v{version}");
    }
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
    let v1 = [
        with_item("  - id: config\n    label: Dup\n    paths: [{ root: home, rel: x }]\n"),
        BASE.replacen("[\"**/credentials\"]", "[\"[unclosed\"]", 1),
        // sensitive path under a non-sensitive item without a matching exclude
        with_item(
            "  - id: creds\n    label: Creds\n    paths: [{ root: user_config, rel: git/token }]\n    flags: { sensitive: true }\n",
        ),
    ];
    let every_version = v1.into_iter().flat_map(|yaml| {
        let v2 = yaml.replacen("schema: 1", "schema: 2", 1);
        let v3 = yaml.replacen("schema: 1", "schema: 3", 1);
        [yaml, v2, v3]
    });
    for yaml in every_version.chain(validator_only_v3()) {
        let json: serde_json::Value = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(schemas.errors(&json), Vec::<String>::new(), "{yaml}");
        let r = load_str(&yaml, Path::new("t")).unwrap();
        assert_eq!(validate(&r).len(), 1, "{yaml}\n{:#?}", validate(&r));
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
