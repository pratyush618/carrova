//! A root removed from the trusted list revokes the installed bundle.

use carrova_bundle::{BundleError, BundleVersion, DirFetcher, UpdateOutcome, Updater};
use carrova_crypto::sign::{SignError, TrustRoots};

use crate::support::{Keys, NOW, bundle, platform, store};

#[test]
fn installed_bundle_from_a_removed_root_is_rejected_on_load() {
    let tmp = tempfile::tempdir().unwrap();
    let (old, new) = (Keys::new(), Keys::new());
    let out = tmp.path().join("v1");
    bundle(&old, tmp.path(), &out, 1, "Alpha");
    let p = platform(&tmp.path().join("home"));
    Updater::new(DirFetcher::new(&out), old.roots.clone())
        .update(&store(&p, &old.roots), NOW)
        .unwrap();

    let err = store(&p, &new.roots).installed().unwrap_err();
    assert!(
        matches!(
            err,
            BundleError::InstalledUntrusted {
                version: BundleVersion(1),
                source: SignError::UnknownRoot(_),
            }
        ),
        "{err}"
    );
}

#[test]
fn installed_bundle_survives_root_rotation_while_its_root_is_listed() {
    let tmp = tempfile::tempdir().unwrap();
    let (old, new) = (Keys::new(), Keys::new());
    let out = tmp.path().join("v1");
    bundle(&old, tmp.path(), &out, 1, "Alpha");
    let p = platform(&tmp.path().join("home"));
    Updater::new(DirFetcher::new(&out), old.roots.clone())
        .update(&store(&p, &old.roots), NOW)
        .unwrap();
    let both = TrustRoots::new(old.roots.iter().chain(new.roots.iter()).copied()).unwrap();
    let installed = store(&p, &both).installed().unwrap().unwrap();
    assert_eq!(installed.version(), BundleVersion(1));
}

#[test]
fn a_revoked_huge_version_does_not_block_legitimate_updates() {
    let tmp = tempfile::tempdir().unwrap();
    let (compromised, legit) = (Keys::new(), Keys::new());
    let (evil, good) = (tmp.path().join("evil"), tmp.path().join("good"));
    bundle(&compromised, tmp.path(), &evil, 900, "Evil");
    bundle(&legit, tmp.path(), &good, 5, "Good");
    let p = platform(&tmp.path().join("home"));
    Updater::new(DirFetcher::new(&evil), compromised.roots.clone())
        .update(&store(&p, &compromised.roots), NOW)
        .unwrap();

    let store = store(&p, &legit.roots);
    let outcome = Updater::new(DirFetcher::new(&good), legit.roots.clone())
        .update(&store, NOW)
        .unwrap();
    assert!(
        matches!(outcome, UpdateOutcome::Installed { .. }),
        "{outcome:?}"
    );
    let installed = store.installed().unwrap().unwrap();
    assert_eq!(installed.version(), BundleVersion(5));
    assert_eq!(installed.previous, None, "the revoked bundle is not kept");
}
