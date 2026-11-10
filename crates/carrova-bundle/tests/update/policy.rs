//! Version-jump cap and creation-time monotonicity.

use carrova_bundle::{
    BundleError, BundleVersion, DirFetcher, MAX_VERSION_JUMP, UpdateCheck, Updater,
};

use crate::support::{Keys, NOW, bundle, bundle_at, platform, store};

/// Store with bundle `installed` (created at `NOW`) installed.
fn installed_at(
    keys: &Keys,
    scratch: &std::path::Path,
    installed: u64,
) -> carrova_platform::fake::FakePlatform {
    let dir = scratch.join(format!("installed-{installed}"));
    bundle(keys, scratch, &dir, installed, "Installed");
    let p = platform(&scratch.join("home"));
    Updater::new(DirFetcher::new(&dir), keys.roots.clone())
        .update(&store(&p, &keys.roots), NOW)
        .unwrap();
    p
}

#[test]
fn a_jump_beyond_the_cap_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let p = installed_at(&keys, tmp.path(), 1);
    let far = tmp.path().join("far");
    bundle(&keys, tmp.path(), &far, 1 + MAX_VERSION_JUMP + 1, "Far");
    let err = Updater::new(DirFetcher::new(&far), keys.roots.clone())
        .check(&store(&p, &keys.roots), NOW)
        .unwrap_err();
    assert!(
        matches!(
            err,
            BundleError::VersionJump {
                installed: BundleVersion(1),
                max: MAX_VERSION_JUMP,
                ..
            }
        ),
        "{err}"
    );
}

#[test]
fn a_jump_of_exactly_the_cap_is_accepted() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let p = installed_at(&keys, tmp.path(), 1);
    let edge = tmp.path().join("edge");
    bundle(&keys, tmp.path(), &edge, 1 + MAX_VERSION_JUMP, "Edge");
    let check = Updater::new(DirFetcher::new(&edge), keys.roots.clone())
        .check(&store(&p, &keys.roots), NOW)
        .unwrap();
    assert!(matches!(check, UpdateCheck::Available(_)), "{check:?}");
}

#[test]
fn any_version_is_accepted_when_nothing_is_installed() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let first = tmp.path().join("first");
    bundle(&keys, tmp.path(), &first, 50_000, "First");
    let p = platform(&tmp.path().join("home"));
    let check = Updater::new(DirFetcher::new(&first), keys.roots.clone())
        .check(&store(&p, &keys.roots), NOW)
        .unwrap();
    assert!(matches!(check, UpdateCheck::Available(_)), "{check:?}");
}

#[test]
fn a_newer_version_created_earlier_than_the_installed_one_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let p = installed_at(&keys, tmp.path(), 1);
    let stale = tmp.path().join("stale");
    bundle_at(&keys, tmp.path(), &stale, 2, "Stale", NOW - 1);
    let err = Updater::new(DirFetcher::new(&stale), keys.roots.clone())
        .check(&store(&p, &keys.roots), NOW)
        .unwrap_err();
    assert!(
        matches!(
            err,
            BundleError::CreatedBackwards {
                offered: c,
                installed: i,
            } if c == NOW - 1 && i == NOW
        ),
        "{err}"
    );
}
