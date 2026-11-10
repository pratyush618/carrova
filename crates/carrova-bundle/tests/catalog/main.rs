//! Client-side validation: every recipe loaded from a bundle passes the
//! same `carrova_recipe::validate` policy as at build time, or is skipped
//! with its reason.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../bundle/support.rs"]
pub mod support;

use std::fs;
use std::path::Path;

use carrova_bundle::{
    Asset, BlobKind, BlobRef, DirFetcher, RecipeSources, SkipCause, Updater, resolve,
};

use support::{Keys, NOW, bundle, edit_index, platform, store};

/// A recipe that parses but breaks a validation rule (sensitive item on
/// by default).
const INVALID: &str = "schema: 1\nid: alpha\nname: alpha\ncategory: tools\n\
    detect:\n  linux:\n    - binary: alpha\n\
    items:\n  - id: keys\n    label: Keys\n    \
    paths: [{ root: user_config, rel: alpha }]\n    \
    flags:\n      sensitive: true\n      default: true\n";

/// Swap `alpha`'s blob for [`INVALID`] and re-sign the index, as a
/// compromised signer could.
fn smuggle_invalid_alpha(keys: &Keys, dir: &Path) {
    let blob = BlobRef::of(INVALID.as_bytes());
    fs::write(
        dir.join(Asset::Blob(BlobKind::Recipe, blob.hash).file_name()),
        INVALID,
    )
    .unwrap();
    edit_index(keys, dir, |index| {
        let alpha = index.recipes.iter_mut().find(|e| e.id == "alpha").unwrap();
        alpha.hash = blob.hash;
        alpha.size = blob.size;
    });
}

fn assert_alpha_skipped_as_invalid(skipped: &[carrova_bundle::SkippedRecipe]) {
    let [only] = skipped else {
        panic!("expected one skipped recipe, got {skipped:?}");
    };
    assert_eq!(only.id, "alpha");
    let SkipCause::Invalid { issues } = &only.reason else {
        panic!("expected a validation skip, got {:?}", only.reason);
    };
    assert!(
        issues
            .iter()
            .any(|i| i.contains("sensitive item must not be on by default")),
        "{issues:?}"
    );
    assert!(only.reason.to_string().contains("fails validation"));
}

#[test]
fn an_invalid_recipe_in_the_shipped_bundle_is_skipped_with_its_reason() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let shipped = tmp.path().join("shipped");
    bundle(&keys, tmp.path(), &shipped, 1, "Alpha");
    smuggle_invalid_alpha(&keys, &shipped);
    let resolved = resolve(&RecipeSources {
        dev_dir: None,
        store: None,
        shipped_dir: Some(&shipped),
        shipped_pin: None,
        dev_fallback: None,
        roots: &keys.roots,
    })
    .unwrap();
    let ids: Vec<&str> = resolved
        .recipes
        .iter()
        .map(|r| r.recipe.id.as_str())
        .collect();
    assert_eq!(ids, ["beta"]);
    assert_alpha_skipped_as_invalid(&resolved.skipped);
}

#[test]
fn an_invalid_recipe_in_an_installed_bundle_is_skipped_and_reported_by_the_update() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let remote = tmp.path().join("remote");
    bundle(&keys, tmp.path(), &remote, 1, "Alpha");
    smuggle_invalid_alpha(&keys, &remote);
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let outcome = Updater::new(DirFetcher::new(&remote), keys.roots.clone())
        .update(&store, NOW)
        .unwrap();
    let carrova_bundle::UpdateOutcome::Installed { skipped, .. } = outcome else {
        panic!("expected an install, got {outcome:?}");
    };
    assert_alpha_skipped_as_invalid(&skipped);
    let catalog = store.recipes().unwrap();
    assert_eq!(catalog.recipes.len(), 1);
    assert_alpha_skipped_as_invalid(&catalog.skipped);
}
