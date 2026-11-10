//! Recipe source resolution: shipped-bundle pinning and the fallback from
//! a revoked installed bundle.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../bundle/support.rs"]
pub mod support;

use std::fs;

use carrova_bundle::{
    Asset, BlobHash, BundleError, BundleVersion, DirFetcher, InstalledRejected, RecipeSource,
    RecipeSources, Updater, resolve, shipped_index_pin,
};
use carrova_crypto::sign::SignError;

use support::{Keys, NOW, bundle, platform, store};

fn shipped_only<'a>(
    shipped: &'a std::path::Path,
    pin: Option<BlobHash>,
    keys: &'a Keys,
) -> RecipeSources<'a> {
    RecipeSources {
        dev_dir: None,
        store: None,
        shipped_dir: Some(shipped),
        shipped_pin: pin,
        dev_fallback: None,
        roots: &keys.roots,
    }
}

#[test]
fn a_shipped_bundle_matching_its_pin_is_used() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let shipped = tmp.path().join("shipped");
    bundle(&keys, tmp.path(), &shipped, 3, "Alpha");
    let pin = BlobHash::of(&fs::read(shipped.join(Asset::INDEX_FILE)).unwrap());
    let resolved = resolve(&shipped_only(&shipped, Some(pin), &keys)).unwrap();
    assert!(matches!(
        resolved.source,
        RecipeSource::Shipped {
            version: BundleVersion(3),
            ..
        }
    ));
}

#[test]
fn a_validly_signed_shipped_bundle_other_than_the_pinned_one_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (pinned, swapped) = (tmp.path().join("pinned"), tmp.path().join("swapped"));
    bundle(&keys, tmp.path(), &pinned, 3, "Alpha");
    bundle(&keys, tmp.path(), &swapped, 2, "Older alpha");
    let pin = BlobHash::of(&fs::read(pinned.join(Asset::INDEX_FILE)).unwrap());
    let err = resolve(&shipped_only(&swapped, Some(pin), &keys)).unwrap_err();
    assert!(
        matches!(err, BundleError::ShippedPinMismatch { expected, .. } if expected == pin),
        "{err}"
    );
}

#[test]
fn builds_without_a_pin_accept_the_shipped_bundle_by_its_chain_alone() {
    assert_eq!(shipped_index_pin(), None, "tests are built without a pin");
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let shipped = tmp.path().join("shipped");
    bundle(&keys, tmp.path(), &shipped, 1, "Alpha");
    assert!(resolve(&shipped_only(&shipped, None, &keys)).is_ok());
}

#[test]
fn a_revoked_installed_bundle_falls_back_to_shipped_with_the_reason() {
    let tmp = tempfile::tempdir().unwrap();
    let (old, new) = (Keys::new(), Keys::new());
    let (installed, shipped) = (tmp.path().join("installed"), tmp.path().join("shipped"));
    bundle(&old, tmp.path(), &installed, 7, "Old");
    bundle(&new, tmp.path(), &shipped, 2, "Shipped");
    let p = platform(&tmp.path().join("home"));
    Updater::new(DirFetcher::new(&installed), old.roots.clone())
        .update(&store(&p, &old.roots), NOW)
        .unwrap();

    let store = store(&p, &new.roots);
    let resolved = resolve(&RecipeSources {
        store: Some(&store),
        ..shipped_only(&shipped, None, &new)
    })
    .unwrap();
    assert!(matches!(
        resolved.source,
        RecipeSource::Shipped {
            version: BundleVersion(2),
            ..
        }
    ));
    assert!(
        matches!(
            resolved.rejected_install,
            Some(InstalledRejected::Untrusted {
                version: BundleVersion(7),
                reason: SignError::UnknownRoot(_),
            })
        ),
        "{:?}",
        resolved.rejected_install
    );
}
