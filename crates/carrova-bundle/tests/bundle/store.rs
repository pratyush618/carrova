use std::fs;

use carrova_bundle::{
    BundleError, BundleVersion, DirFetcher, InstallOutcome, UpdateCheck, Updater,
};

use crate::support::{Keys, NOW, bundle, platform, store};

fn description(store: &carrova_bundle::BundleStore<'_>) -> String {
    let catalog = store.recipes().unwrap();
    catalog.recipes[0].recipe.description.clone().unwrap()
}

#[test]
fn rollback_restores_previous_index_and_prunes_the_newer() {
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
    let installed = store.installed().unwrap().unwrap();
    assert_eq!(installed.version(), BundleVersion(2));
    assert_eq!(installed.previous, Some(BundleVersion(1)));
    assert_eq!(description(&store), "Alpha two");
    let blobs_before = fs::read_dir(store.root().join("blobs")).unwrap().count();

    assert_eq!(store.rollback().unwrap(), BundleVersion(1));
    let installed = store.installed().unwrap().unwrap();
    assert_eq!(
        (installed.version(), installed.previous),
        (BundleVersion(1), None)
    );
    assert_eq!(description(&store), "Alpha");
    let blobs_after = fs::read_dir(store.root().join("blobs")).unwrap().count();
    assert_eq!(blobs_after, blobs_before - 1, "v2-only blob pruned");
    assert!(!store.root().join("indexes/2").exists());
    assert!(matches!(store.rollback(), Err(BundleError::NoPrevious)));
}

#[test]
fn installing_the_current_version_again_is_a_no_op() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let updater = Updater::new(DirFetcher::new(&out), keys.roots.clone());
    let UpdateCheck::Available(pending) = updater.check(&store, NOW).unwrap() else {
        panic!("expected update");
    };
    updater.fetch(&pending, &store).unwrap();
    store.install(pending.signed()).unwrap();
    assert_eq!(
        store.install(pending.signed()).unwrap(),
        InstallOutcome::AlreadyCurrent(BundleVersion(1))
    );
}

#[test]
fn damaged_stored_blob_fails_loading() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    Updater::new(DirFetcher::new(&out), keys.roots.clone())
        .update(&store, NOW)
        .unwrap();
    let blobs = store.root().join("blobs");
    for entry in fs::read_dir(&blobs).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "yaml") {
            fs::write(&path, b"schema: 1\n").unwrap();
        }
    }
    let err = store.recipes().unwrap_err();
    assert!(matches!(err, BundleError::BlobMismatch { .. }), "{err}");
}

#[test]
fn install_without_fetched_blobs_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let updater = Updater::new(DirFetcher::new(&out), keys.roots.clone());
    let UpdateCheck::Available(pending) = updater.check(&store, NOW).unwrap() else {
        panic!("expected update");
    };
    let err = store.install(pending.signed()).unwrap_err();
    assert!(matches!(err, BundleError::MissingBlob { .. }), "{err}");
    assert_eq!(store.installed().unwrap(), None);
}
