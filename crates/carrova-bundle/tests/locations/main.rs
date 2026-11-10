//! `RecipeLocations`: shipped-bundle detection and source resolution order.

#![allow(clippy::unwrap_used)]

use std::path::Path;

use carrova_bundle::{Asset, BundleError, RecipeLocations, RecipeSource, TrustRoots};
use carrova_crypto::sign::SigningKey;
use carrova_platform::fake::FakePlatform;

fn roots() -> TrustRoots {
    TrustRoots::new([SigningKey::generate().unwrap().verifying_key()]).unwrap()
}

fn write_recipe(tree: &Path) {
    let dir = tree.join("tools/alpha");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("recipe.yaml"),
        "schema: 1\nid: alpha\nname: Alpha\ncategory: tools\n\
         detect:\n  linux:\n    - binary: alpha\n\
         items:\n  - id: config\n    label: Config\n    paths: [{ root: user_config, rel: alpha }]\n",
    )
    .unwrap();
}

#[test]
fn a_dir_without_an_index_is_not_a_shipped_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(RecipeLocations::shipped_bundle(tmp.path().into()), None);
    std::fs::write(tmp.path().join(Asset::INDEX_FILE), "{}").unwrap();
    assert_eq!(
        RecipeLocations::shipped_bundle(tmp.path().into()),
        Some(tmp.path().to_path_buf())
    );
}

#[test]
fn the_fallback_tree_is_used_when_nothing_else_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let tree = tmp.path().join("recipes");
    write_recipe(&tree);
    let platform = FakePlatform::with_home(tmp.path().join("home"));
    let locations = RecipeLocations {
        dev_fallback: Some(tree.clone()),
        ..RecipeLocations::default()
    };
    let resolved = locations.resolve(&platform, &roots()).unwrap();
    assert_eq!(resolved.source, RecipeSource::DevDir(tree));
    assert_eq!(resolved.recipes[0].recipe.id, "alpha");
}

#[test]
fn no_location_means_no_recipes() {
    let tmp = tempfile::tempdir().unwrap();
    let platform = FakePlatform::with_home(tmp.path().join("home"));
    let err = RecipeLocations::default()
        .resolve(&platform, &roots())
        .unwrap_err();
    assert!(matches!(err, BundleError::NoRecipes), "{err}");
}
