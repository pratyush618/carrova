use super::*;

#[test]
fn invalid_recipes_are_dropped_and_reported() {
    let tmp = tempfile::TempDir::new().unwrap();
    let good = tmp.path().join("dev/good");
    let bad = tmp.path().join("dev/bad");
    let broken = tmp.path().join("dev/broken");
    for d in [&good, &bad, &broken] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(
        good.join("recipe.yaml"),
        "schema: 1\nid: good\nname: Good\ncategory: dev\ndetect:\n  linux:\n    - binary: good\nitems:\n  - id: cfg\n    label: Config\n    paths:\n      - { root: user_config, rel: good }\n",
    )
    .unwrap();
    // Validation issue: id does not match directory.
    std::fs::write(
        bad.join("recipe.yaml"),
        "schema: 1\nid: other\nname: Bad\ncategory: dev\ndetect:\n  linux:\n    - binary: bad\nitems:\n  - id: cfg\n    label: Config\n    paths:\n      - { root: user_config, rel: bad }\n",
    )
    .unwrap();
    std::fs::write(broken.join("recipe.yaml"), "not: [valid").unwrap();

    let cat = load_catalog(tmp.path()).unwrap();
    let ids: Vec<&str> = cat.recipes.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["good"]);
    assert_eq!(cat.problems.len(), 2, "{:?}", cat.problems);
}

fn recipe_yaml(id: &str, category: &str) -> String {
    format!(
        "schema: 1\nid: {id}\nname: {id}\ncategory: {category}\ndetect:\n  linux:\n    - binary: {id}\nitems:\n  - id: cfg\n    label: Config\n    paths:\n      - {{ root: user_config, rel: {id} }}\n"
    )
}

/// `<tmp>/<category_dir>/<id>/recipe.yaml`; returns the app directory.
fn write_recipe(root: &Path, category_dir: &str, id: &str, category: &str) -> PathBuf {
    let dir = root.join(category_dir).join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(RECIPE_FILE), recipe_yaml(id, category)).unwrap();
    dir
}

#[test]
fn validate_single_dir() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = write_recipe(tmp.path(), "dev", "good", "dev");
    let r = validate_targets(&[dir]);
    assert_eq!(r.recipes_checked, 1);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn validate_single_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = write_recipe(tmp.path(), "dev", "good", "dev");
    let r = validate_targets(&[dir.join(RECIPE_FILE)]);
    assert_eq!(r.recipes_checked, 1);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn validate_tree_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_recipe(tmp.path(), "dev", "one", "dev");
    write_recipe(tmp.path(), "shells", "two", "shells");
    let r = validate_targets(&[tmp.path().to_path_buf()]);
    assert_eq!(r.recipes_checked, 2);
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn validate_dedups_by_path() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = write_recipe(tmp.path(), "dev", "good", "dev");
    let r = validate_targets(&[tmp.path().to_path_buf(), dir.clone(), dir.join(RECIPE_FILE)]);
    assert_eq!(r.recipes_checked, 1);
    // A duplicate would also surface as a duplicate-id issue.
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn validate_single_recipe_keeps_layout_checks() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = write_recipe(tmp.path(), "shells", "bad", "wrong");
    for target in [dir.clone(), dir.join(RECIPE_FILE)] {
        let r = validate_targets(&[target]);
        assert_eq!(r.recipes_checked, 1);
        assert_eq!(r.issues.len(), 1, "{r:?}");
        assert!(r.issues[0].contains("`wrong`"), "{r:?}");
        assert!(r.issues[0].contains("`shells`"), "{r:?}");
        assert!(!r.is_ok());
    }
}

#[test]
fn layout_inferred_from_parent_dirs() {
    let layout = infer_layout(Path::new("recipes/dev/git/recipe.yaml")).unwrap();
    assert_eq!(layout.category_dir, "dev");
    assert_eq!(layout.app_dir, "git");
    assert!(infer_layout(Path::new("recipe.yaml")).is_none());
}

#[test]
fn validate_unparseable_recipe_is_a_load_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    write_recipe(tmp.path(), "dev", "good", "dev");
    let broken = tmp.path().join("dev/broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join(RECIPE_FILE), "id: [oops").unwrap();
    let r = validate_targets(&[tmp.path().to_path_buf()]);
    assert_eq!(r.recipes_checked, 1);
    assert_eq!(r.load_errors.len(), 1, "{r:?}");
    assert!(r.load_errors[0].contains("broken"), "{r:?}");
    assert!(r.issues.is_empty(), "{r:?}");
    assert!(!r.is_ok());
}

#[test]
fn validate_missing_target_is_a_load_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let r = validate_targets(&[tmp.path().join("nope")]);
    assert_eq!(r.recipes_checked, 0);
    assert_eq!(r.load_errors.len(), 1, "{r:?}");
    assert!(!r.is_ok());
}

#[test]
fn validation_report_serializes() {
    let r = ValidationReport {
        recipes_checked: 2,
        load_errors: vec![],
        issues: vec!["x".into()],
    };
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["recipes_checked"], 2);
    assert_eq!(v["issues"][0], "x");
}

#[test]
fn missing_root_is_an_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(load_catalog(&tmp.path().join("nope")).is_err());
}
