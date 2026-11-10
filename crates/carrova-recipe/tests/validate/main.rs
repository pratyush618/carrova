//! Recipe validation policy and the issues it reports.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use carrova_recipe::{
    DetectRule, IssueKind, LoadedRecipe, Recipe, TreeLayout, is_kebab, load_str, validate,
    validate_tree,
};

const BASE: &str = r#"
schema: 1
id: demo
name: Demo
category: dev-tools
detect:
  linux:
    - binary: demo
items:
  - id: config
    label: Config
    paths: [{ root: user_config, rel: demo }]
"#;

fn recipe(text: &str) -> Recipe {
    load_str(text, Path::new("t.yaml")).unwrap()
}

#[test]
fn kebab() {
    for ok in ["a", "git", "vs-code", "a1-b2-c3", "7zip"] {
        assert!(is_kebab(ok), "{ok}");
    }
    for bad in ["", "-a", "a-", "a--b", "Git", "vs_code", "a b", "ä"] {
        assert!(!is_kebab(bad), "{bad}");
    }
}

#[test]
fn base_is_clean() {
    assert_eq!(validate(&recipe(BASE)), vec![]);
}

#[test]
fn collects_all_issues() {
    let mut r = recipe(BASE);
    r.schema = 3;
    r.id = "Demo_App".into();
    r.category = "Dev Tools".into();
    r.name = " ".into();
    r.detect.linux = vec![DetectRule::Binary("/usr/bin/demo".into())];
    let mut dup = r.items[0].clone();
    dup.exclude = vec!["a/**".into(), "[unclosed".into()];
    dup.flags.sensitive = true;
    dup.flags.default = Some(true);
    r.items.push(dup);
    let mut unm = r.items[0].clone();
    unm.id = "keyring".into();
    unm.unmovable = Some("OS-bound".into());
    unm.flags.default = Some(true);
    r.items.push(unm);
    let mut nopaths = r.items[0].clone();
    nopaths.id = "Bad Item".into();
    nopaths.paths.clear();
    nopaths.label = String::new();
    r.items.push(nopaths);

    let kinds: Vec<(String, IssueKind)> = validate(&r)
        .into_iter()
        .map(|i| (i.location, i.kind))
        .collect();
    let expect = [
        ("schema", IssueKind::UnsupportedSchema { found: 3 }),
        (
            "id",
            IssueKind::NotKebab {
                value: "Demo_App".into(),
            },
        ),
        ("name", IssueKind::Empty),
        (
            "category",
            IssueKind::NotKebab {
                value: "Dev Tools".into(),
            },
        ),
        (
            "detect.linux[0].binary",
            IssueKind::InvalidBinaryName {
                name: "/usr/bin/demo".into(),
            },
        ),
        (
            "items[1].id",
            IssueKind::DuplicateItemId {
                id: "config".into(),
            },
        ),
        ("items[1].flags.default", IssueKind::SensitiveDefaultOn),
        ("items[2].flags.default", IssueKind::UnmovableDefaultOn),
        (
            "items[3].id",
            IssueKind::NotKebab {
                value: "Bad Item".into(),
            },
        ),
        ("items[3].label", IssueKind::Empty),
        ("items[3].paths", IssueKind::NoPaths),
        (
            "items[0].paths[0]",
            IssueKind::SensitiveCapturedByNonSensitive {
                path: r.items[1].paths[0].clone(),
                sensitive_item: "config".into(),
                by_item: "config".into(),
            },
        ),
    ];
    for (loc, kind) in &expect {
        assert!(
            kinds.iter().any(|(l, k)| l == loc && k == kind),
            "missing {loc}: {kind:?} in {kinds:#?}"
        );
    }
    assert!(kinds.iter().any(|(l, k)| l == "items[1].exclude[1]"
        && matches!(k, IssueKind::InvalidGlob { glob, .. } if glob == "[unclosed")));
    assert_eq!(kinds.len(), expect.len() + 1, "{kinds:#?}");
}

#[test]
fn unmovable_without_paths_is_fine() {
    let text = format!(
        "{BASE}  - id: passwords\n    label: Saved passwords\n    unmovable: Encrypted with the OS keyring\n"
    );
    let r = recipe(&text);
    assert_eq!(validate(&r), vec![]);
    assert!(!r.items[1].default_selected());
    assert!(!r.items[1].is_movable());
}

#[test]
fn v2_fields() {
    use carrova_recipe::Handler;
    let mut r = recipe(&BASE.replace("schema: 1", "schema: 2"));
    r.processes = vec!["demo-agent".into()];
    r.items[0].handler = Some(Handler::GpgKeyring);
    assert_eq!(validate(&r), vec![]);

    r.processes = vec![String::new(), "/usr/bin/x".into(), "ok".into()];
    let mut unm = r.items[0].clone();
    unm.id = "kr".into();
    unm.unmovable = Some("OS-bound".into());
    r.items.push(unm);
    let kinds: Vec<(String, IssueKind)> = validate(&r)
        .into_iter()
        .map(|i| (i.location, i.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (
                "processes[0]".into(),
                IssueKind::InvalidProcessName {
                    name: String::new()
                }
            ),
            (
                "processes[1]".into(),
                IssueKind::InvalidProcessName {
                    name: "/usr/bin/x".into()
                }
            ),
            ("items[1].handler".into(), IssueKind::UnmovableWithHandler),
        ]
    );
}

#[test]
fn v2_fields_rejected_in_v1() {
    let mut r = recipe(BASE);
    r.processes = vec!["demo-agent".into()];
    r.items[0].handler = Some(carrova_recipe::Handler::GpgKeyring);
    let kinds: Vec<(String, IssueKind)> = validate(&r)
        .into_iter()
        .map(|i| (i.location, i.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (
                "processes".into(),
                IssueKind::RequiresSchema {
                    field: "processes",
                    min: 2
                }
            ),
            (
                "items[0].handler".into(),
                IssueKind::RequiresSchema {
                    field: "handler",
                    min: 2
                }
            ),
        ]
    );
}

const OVERLAP: &str = r#"
schema: 1
id: demo
name: Demo
category: shells
detect:
  linux:
    - binary: demo
items:
  - id: config
    label: Config
    paths:
      - { root: home, rel: .demo }
      - { root: user_config, rel: demo }
    exclude: ["**/history", "secrets/*.key", ".demo/tokens"]
  - id: history
    label: History
    paths:
      - { root: home, rel: .demo/history }
      - { root: user_config, rel: demo/sub/history }
      - { root: user_config, rel: demo/secrets/a.key }
      - { root: home, rel: .demo/tokens/t1 }
      - { root: home, rel: .elsewhere }
      - { root: cache, rel: demo }
    flags: { sensitive: true }
"#;

fn captured(r: &Recipe) -> Vec<(String, String)> {
    validate(r)
        .into_iter()
        .map(|i| match i.kind {
            IssueKind::SensitiveCapturedByNonSensitive { path, by_item, .. } => {
                (i.location, format!("{path} by {by_item}"))
            }
            other => (i.location, format!("unexpected {other:?}")),
        })
        .collect()
}

#[test]
fn sensitive_paths_excluded_from_non_sensitive_items() {
    // item-relative glob, root-relative glob, excluded ancestor dir,
    // unrelated path, other root: all fine.
    assert_eq!(captured(&recipe(OVERLAP)), vec![]);
}

#[test]
fn sensitive_path_captured_without_exclude_is_reported() {
    // Regression: removing an exclude glob silently leaked sensitive
    // files into a default-on item.
    let r = recipe(&OVERLAP.replace("\"**/history\", ", ""));
    assert_eq!(
        captured(&r),
        vec![
            (
                "items[0].paths[0]".into(),
                "home:.demo/history by config".into()
            ),
            (
                "items[0].paths[1]".into(),
                "user_config:demo/sub/history by config".into()
            ),
        ]
    );
    // Exact same path in both items.
    let r = recipe(&OVERLAP.replace("rel: .elsewhere", "rel: .demo"));
    assert_eq!(
        captured(&r),
        vec![("items[0].paths[0]".into(), "home:.demo by config".into())]
    );
}

#[test]
fn overlap_ignored_for_sensitive_or_unmovable_capturers() {
    let base = OVERLAP.replace(
        "exclude: [\"**/history\", \"secrets/*.key\", \".demo/tokens\"]",
        "",
    );
    let mut r = recipe(&base);
    assert_eq!(captured(&r).len(), 4, "{:?}", captured(&r));
    r.items[0].flags.sensitive = true;
    assert_eq!(captured(&r), vec![]);
    r.items[0].flags.sensitive = false;
    r.items[0].unmovable = Some("OS-bound".into());
    assert_eq!(captured(&r), vec![]);
}

fn loaded(path: &str, text: &str, cat: &str, app: &str) -> LoadedRecipe {
    LoadedRecipe {
        path: PathBuf::from(path),
        layout: Some(TreeLayout {
            category_dir: cat.into(),
            app_dir: app.into(),
        }),
        recipe: recipe(text),
    }
}

#[test]
fn tree_checks() {
    let a = loaded("r/dev-tools/demo/recipe.yaml", BASE, "dev-tools", "demo");
    let b = loaded("r/shells/demo2/recipe.yaml", BASE, "shells", "demo2");
    let issues = validate_tree(&[a, b]);
    let kinds: Vec<&IssueKind> = issues.iter().map(|i| &i.kind).collect();
    assert_eq!(
        kinds,
        vec![
            &IssueKind::CategoryDirMismatch {
                category: "dev-tools".into(),
                dir: "shells".into()
            },
            &IssueKind::AppDirMismatch {
                id: "demo".into(),
                dir: "demo2".into()
            },
            &IssueKind::DuplicateRecipeId {
                id: "demo".into(),
                other: PathBuf::from("r/dev-tools/demo/recipe.yaml")
            },
        ]
    );
    let line = issues[2].to_string();
    assert!(
        line.starts_with("r/shells/demo2/recipe.yaml: [demo] id: "),
        "{line}"
    );
}
