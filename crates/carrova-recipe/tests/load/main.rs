//! Recipe loading: parsing, schema versions and tree layout.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::fs;
use std::path::Path;

use carrova_recipe::{
    DetectRule, LoadError, Portability, SemanticRoot, TreeLayout, load_str, load_tree,
};

const GIT: &str = r#"
schema: 1
id: git
name: Git
category: dev-tools
detect:
  linux:
    - binary: git
    - path: { root: home, rel: .gitconfig }
items:
  - id: config
    label: Global git config
    paths:
      - { root: home, rel: .gitconfig }
      - { root: user_config, rel: ./git }
    exclude: ["**/credentials"]
    flags:
      portability: same_os
      sensitive: false
      requires_closed: false
      default: true
  - id: credentials
    label: Stored credentials
    paths: [{ root: home, rel: .git-credentials }]
    flags: { sensitive: true }
"#;

#[test]
fn parses_example() {
    let r = load_str(GIT, Path::new("git.yaml")).unwrap();
    assert_eq!(r.id, "git");
    assert_eq!(r.detect.linux.len(), 2);
    assert_eq!(r.detect.linux[0], DetectRule::Binary("git".into()));
    let DetectRule::Path(p) = &r.detect.linux[1] else {
        panic!("expected path rule");
    };
    assert_eq!(p.root, SemanticRoot::Home);
    assert_eq!(r.items[0].paths[1].rel.as_str(), "git");
    assert_eq!(r.items[0].flags.portability, Portability::SameOs);
    assert!(r.items[0].default_selected());
    // Sensitive with omitted `default` -> off.
    assert!(!r.items[1].default_selected());
    assert!(carrova_recipe::validate(&r).is_empty());
}

#[test]
fn roundtrips_through_yaml() {
    let r = load_str(GIT, Path::new("git.yaml")).unwrap();
    let text = serde_yaml_ng::to_string(&r).unwrap();
    assert_eq!(load_str(&text, Path::new("x")).unwrap(), r);
}

fn parse_err(text: &str) -> String {
    match load_str(text, Path::new("bad.yaml")) {
        Ok(r) => panic!("accepted: {r:?}"),
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("bad.yaml"), "path missing from {msg}");
            msg
        }
    }
}

#[test]
fn rejects_bad_yaml() {
    let unknown = GIT.replace("name: Git", "name: Git\nhomepage: x");
    assert!(parse_err(&unknown).contains("homepage"));
    let escape = GIT.replace("rel: .gitconfig }\nitems", "rel: ../.gitconfig }\nitems");
    assert!(parse_err(&escape).contains(".."));
    let abs = GIT.replace("rel: ./git", "rel: /etc/git");
    assert!(parse_err(&abs).contains("absolute"));
    let bad_root = GIT.replace("root: user_config", "root: etc");
    parse_err(&bad_root);
    let both = GIT.replace(
        "- binary: git",
        "- { binary: git, path: { root: home, rel: x } }",
    );
    assert!(parse_err(&both).contains("both"));
    let flag = GIT.replace("sensitive: true", "secret: true");
    parse_err(&flag);
    let reserved = GIT.replace(
        "label: Global git config",
        "label: G\n    export: git config -l",
    );
    assert!(parse_err(&reserved).contains("export"));
    parse_err("items: [");
}

#[test]
fn unsupported_schema_is_reported_before_unknown_fields() {
    let v3 = GIT.replace("schema: 1", "schema: 3\nnew_field: x");
    let err = load_str(&v3, Path::new("v3.yaml")).unwrap_err();
    assert!(
        matches!(err, LoadError::UnsupportedSchema { found: 3, .. }),
        "{err}"
    );
}

#[test]
fn parses_v2_fields() {
    let v2 = GIT
        .replace("schema: 1", "schema: 2")
        .replace("items:\n", "processes: [git-daemon]\nitems:\n")
        .replace(
            "label: Stored credentials",
            "label: Stored credentials\n    handler: gpg-keyring",
        );
    let r = load_str(&v2, Path::new("v2.yaml")).unwrap();
    assert_eq!(r.processes, ["git-daemon"]);
    assert_eq!(
        r.items[1].handler,
        Some(carrova_recipe::Handler::GpgKeyring)
    );
    assert_eq!(r.items[0].handler, None);
    let text = serde_yaml_ng::to_string(&r).unwrap();
    assert!(text.contains("handler: gpg-keyring"), "{text}");
    assert_eq!(load_str(&text, Path::new("x")).unwrap(), r);

    let unknown = v2.replace("handler: gpg-keyring", "handler: gpg-export");
    assert!(parse_err(&unknown).contains("gpg-export"));
}

#[test]
fn handler_names_are_stable() {
    for h in carrova_recipe::Handler::ALL {
        assert_eq!(carrova_recipe::Handler::from_name(h.as_str()), Some(*h));
        assert_eq!(h.to_string(), h.as_str());
        let yaml = serde_yaml_ng::to_string(h).unwrap();
        assert_eq!(yaml.trim(), h.as_str());
    }
    assert_eq!(carrova_recipe::Handler::GpgKeyring.as_str(), "gpg-keyring");
    assert_eq!(carrova_recipe::Handler::from_name("nope"), None);
}

#[test]
fn process_names_cover_binaries_and_processes() {
    use carrova_platform::OsKind;
    let v2 = GIT.replace("schema: 1", "schema: 2").replace(
        "items:\n",
        "  macos:\n    - binary: git-mac\nprocesses: [git-daemon]\nitems:\n",
    );
    let r = load_str(&v2, Path::new("v2.yaml")).unwrap_or_else(|e| panic!("{e}\n{v2}"));
    let linux: Vec<&str> = r.process_names(OsKind::Linux).collect();
    assert_eq!(linux, ["git", "git-daemon"]);
    let mac: Vec<&str> = r.process_names(OsKind::MacOs).collect();
    assert_eq!(mac, ["git-mac", "git-daemon"]);
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn loads_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(&root.join("dev-tools/git/recipe.yaml"), GIT);
    // fixture subtree must be ignored even if it contains recipe-like files
    write(
        &root.join("dev-tools/git/fixture/home/recipe.yaml"),
        "garbage: [",
    );
    write(&root.join("schema/recipe.v1.json"), "{}");
    fs::create_dir_all(root.join("schema/nested")).unwrap();
    write(&root.join("README.md"), "hi");
    write(&root.join("shells/bash/recipe.yaml"), "schema: 1\n");
    fs::create_dir_all(root.join("shells/empty")).unwrap();
    fs::create_dir_all(root.join(".hidden/x")).unwrap();

    let tree = load_tree(root).unwrap();
    assert_eq!(tree.recipes.len(), 1);
    let git = &tree.recipes[0];
    assert_eq!(git.recipe.id, "git");
    assert_eq!(git.dir(), root.join("dev-tools/git"));
    assert_eq!(
        git.layout,
        Some(TreeLayout {
            category_dir: "dev-tools".into(),
            app_dir: "git".into()
        })
    );
    assert_eq!(tree.errors.len(), 2, "{:?}", tree.errors);
    assert!(tree.errors.iter().any(
        |e| matches!(e, LoadError::Parse { path, .. } if path.ends_with("shells/bash/recipe.yaml"))
    ));
    assert!(
        tree.errors.iter().any(
            |e| matches!(e, LoadError::MissingRecipe { dir } if dir.ends_with("shells/empty"))
        )
    );
}

#[test]
fn missing_root_is_error() {
    let tmp = tempfile::tempdir().unwrap();
    let err = load_tree(tmp.path().join("nope")).unwrap_err();
    assert!(err.path().ends_with("nope"));
}
