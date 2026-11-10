//! Versions the user rolled back are never offered or installed again.

use carrova_bundle::{BundleError, BundleVersion, DirFetcher, UpdateCheck, UpdateOutcome, Updater};

use crate::support::{Keys, NOW, bundle, platform, store};

#[test]
fn a_rolled_back_version_is_never_offered_or_installed_again() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (v1, v2) = (tmp.path().join("v1"), tmp.path().join("v2"));
    bundle(&keys, tmp.path(), &v1, 1, "Alpha");
    bundle(&keys, tmp.path(), &v2, 2, "Alpha two");
    let p = platform(&tmp.path().join("home"));
    let up = |dir| {
        Updater::new(DirFetcher::new(dir), keys.roots.clone()).update(&store(&p, &keys.roots), NOW)
    };
    up(&v1).unwrap();
    up(&v2).unwrap();
    store(&p, &keys.roots).rollback().unwrap();

    // A fresh store (another process) still remembers the refusal.
    let store = store(&p, &keys.roots);
    let updater = Updater::new(DirFetcher::new(&v2), keys.roots.clone());
    assert!(matches!(
        updater.check(&store, NOW).unwrap(),
        UpdateCheck::Refused(BundleVersion(2))
    ));
    assert!(matches!(
        updater.update(&store, NOW).unwrap(),
        UpdateOutcome::Refused(BundleVersion(2))
    ));
    let UpdateCheck::Refused(_) = updater.check(&store, NOW).unwrap() else {
        panic!("refusal forgotten");
    };
    let pending = Updater::new(DirFetcher::new(&v1), keys.roots.clone());
    assert!(matches!(
        pending.check(&store, NOW).unwrap(),
        UpdateCheck::UpToDate(BundleVersion(1))
    ));
    assert_eq!(
        store.installed().unwrap().unwrap().version(),
        BundleVersion(1)
    );
}

#[test]
fn installing_a_refused_version_directly_fails() {
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
    let (signed, _) = carrova_bundle::open_bundle(
        &DirFetcher::new(&v2),
        &keys.roots,
        carrova_bundle::VerifyTime::Now(NOW),
    )
    .unwrap();
    let err = store.install(&signed).unwrap_err();
    assert!(
        matches!(err, BundleError::Refused(BundleVersion(2))),
        "{err}"
    );
}
