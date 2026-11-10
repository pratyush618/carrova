//! `BundleStore::reset`: back to the shipped bundle.

use carrova_bundle::{
    BundleError, BundleVersion, DirFetcher, RecipeSource, RecipeSources, UpdateCheck, Updater,
    resolve,
};

use crate::support::{Keys, NOW, bundle, platform, store};

#[test]
fn reset_removes_the_installed_bundle_so_the_shipped_one_is_used() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (shipped, v4) = (tmp.path().join("shipped"), tmp.path().join("v4"));
    bundle(&keys, tmp.path(), &shipped, 3, "Shipped");
    bundle(&keys, tmp.path(), &v4, 4, "Four");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    Updater::new(DirFetcher::new(&v4), keys.roots.clone())
        .update(&store, NOW)
        .unwrap();

    assert_eq!(store.reset().unwrap(), Some(BundleVersion(4)));
    assert_eq!(store.installed().unwrap(), None);
    let blobs = std::fs::read_dir(store.root().join("blobs")).unwrap();
    assert_eq!(blobs.count(), 0, "blobs pruned");
    let sources = RecipeSources {
        dev_dir: None,
        store: Some(&store),
        shipped_dir: Some(&shipped),
        shipped_pin: None,
        dev_fallback: None,
        roots: &keys.roots,
    };
    assert!(matches!(
        resolve(&sources).unwrap().source,
        RecipeSource::Shipped {
            version: BundleVersion(3),
            ..
        }
    ));
    assert!(matches!(store.rollback(), Err(BundleError::NotInstalled)));
    assert_eq!(store.reset().unwrap(), None, "reset is idempotent");
}

#[test]
fn reset_keeps_rolled_back_versions_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (v1, v2) = (tmp.path().join("v1"), tmp.path().join("v2"));
    bundle(&keys, tmp.path(), &v1, 1, "Alpha");
    bundle(&keys, tmp.path(), &v2, 2, "Alpha two");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    for dir in [&v1, &v2] {
        Updater::new(DirFetcher::new(dir), keys.roots.clone())
            .update(&store, NOW)
            .unwrap();
    }
    store.rollback().unwrap();
    store.reset().unwrap();
    let check = Updater::new(DirFetcher::new(&v2), keys.roots.clone())
        .check(&store, NOW)
        .unwrap();
    assert!(
        matches!(check, UpdateCheck::Refused(BundleVersion(2))),
        "{check:?}"
    );
}
