//! Reading the store never writes: loading recipes runs before any plan
//! is approved, and nothing may be written on a receiver before that.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use carrova_bundle::{
    BundleError, BundleVersion, DirFetcher, RecipeLocations, RecipeSource, Updater,
};

use crate::support::{Keys, NOW, bundle, platform, store, write_tree};

/// Every path under `dir` (empty if it does not exist).
fn tree(dir: &Path) -> BTreeSet<PathBuf> {
    let mut out = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        out.extend(tree(&path));
        out.insert(path);
    }
    out
}

#[test]
fn opening_and_reading_an_empty_store_creates_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let home = tmp.path().join("home");
    let p = platform(&home);
    let before = tree(&home);

    let store = store(&p, &keys.roots);
    assert_eq!(store.installed().unwrap(), None);
    assert!(matches!(store.recipes(), Err(BundleError::NotInstalled)));
    assert!(matches!(
        store.detect_index(),
        Err(BundleError::NotInstalled)
    ));

    assert_eq!(tree(&home), before, "reading the store wrote to the home");
}

#[test]
fn resolving_recipes_on_an_empty_home_creates_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let checkout = tmp.path().join("recipes");
    write_tree(&checkout, "Alpha");
    let home = tmp.path().join("home");
    let p = platform(&home);
    let before = tree(&home);

    let locations = RecipeLocations {
        dev_fallback: Some(checkout.clone()),
        ..RecipeLocations::default()
    };
    let resolved = locations.resolve(&p, &keys.roots).unwrap();

    assert_eq!(resolved.source, RecipeSource::DevDir(checkout));
    assert_eq!(tree(&home), before, "resolving recipes wrote to the home");
}

#[test]
fn the_first_install_creates_the_store() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let v1 = tmp.path().join("v1");
    bundle(&keys, tmp.path(), &v1, 1, "One");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);

    Updater::new(DirFetcher::new(&v1), keys.roots.clone())
        .update(&store, NOW)
        .unwrap();

    let installed = store.installed().unwrap().expect("installed");
    assert_eq!(installed.version(), BundleVersion(1));
}
