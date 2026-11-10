#![allow(clippy::unwrap_used)]

mod internals;

use carrova_bundle::Asset;

use super::*;

#[test]
fn the_shipped_bundle_is_the_resource_dirs_recipes_when_it_holds_one() {
    let resources = tempfile::tempdir().unwrap();
    assert_eq!(app_locations(Some(resources.path())).shipped_dir, None);
    let bundle = resources.path().join("recipes");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join(Asset::INDEX_FILE), "{}").unwrap();
    assert_eq!(
        app_locations(Some(resources.path())).shipped_dir,
        Some(bundle)
    );
}

#[test]
fn debug_builds_fall_back_to_the_checkout_tree() {
    let fallback = app_locations(None).dev_fallback;
    assert_eq!(fallback.is_some(), cfg!(debug_assertions));
    if let Some(tree) = fallback {
        assert!(tree.ends_with("recipes") && tree.is_dir());
    }
}
