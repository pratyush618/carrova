use carrova_bundle::{
    BundleError, BundleVersion, DirFetcher, RecipeSource, RecipeSources, Updater, VerifyTime,
    open_bundle, resolve,
};
use carrova_crypto::sign::SignError;

use crate::support::{Keys, NOW, bundle, platform, store, write_tree};

#[test]
fn sources_resolve_in_documented_order() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let dev = tmp.path().join("dev");
    write_tree(&dev, "Dev alpha");
    let shipped = tmp.path().join("shipped");
    bundle(&keys, tmp.path(), &shipped, 3, "Shipped alpha");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let mut sources = RecipeSources {
        dev_dir: Some(&dev),
        store: Some(&store),
        shipped_dir: Some(&shipped),
        shipped_pin: None,
        dev_fallback: None,
        roots: &keys.roots,
    };

    let resolved = resolve(&sources).unwrap();
    assert_eq!(resolved.source, RecipeSource::DevDir(dev.clone()));
    assert!(!resolved.source.is_verified());

    sources.dev_dir = None;
    let resolved = resolve(&sources).unwrap();
    assert_eq!(
        resolved.source,
        RecipeSource::Shipped {
            dir: shipped.clone(),
            version: BundleVersion(3)
        },
        "nothing installed yet"
    );

    let remote = tmp.path().join("remote");
    bundle(&keys, tmp.path(), &remote, 5, "Remote alpha");
    Updater::new(DirFetcher::new(&remote), keys.roots.clone())
        .update(&store, NOW)
        .unwrap();
    let resolved = resolve(&sources).unwrap();
    assert_eq!(resolved.source, RecipeSource::Installed(BundleVersion(5)));
    assert_eq!(
        resolved.recipes[0].recipe.description.as_deref(),
        Some("Remote alpha")
    );

    let none = RecipeSources {
        dev_dir: None,
        store: None,
        shipped_dir: None,
        shipped_pin: None,
        dev_fallback: None,
        roots: &keys.roots,
    };
    assert!(matches!(resolve(&none), Err(BundleError::NoRecipes)));
}

#[test]
fn shipped_bundle_is_verified_at_its_signing_time() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let shipped = tmp.path().join("shipped");
    bundle(&keys, tmp.path(), &shipped, 1, "Alpha");
    let years_later = VerifyTime::Now(NOW + 5 * 365 * 86_400);
    let err = open_bundle(&DirFetcher::new(&shipped), &keys.roots, years_later).unwrap_err();
    assert!(
        matches!(err, BundleError::Signature(SignError::Expired { .. })),
        "{err}"
    );
    // Resolution takes no clock: shipped bundles are checked at signing time.
    let sources = RecipeSources {
        dev_dir: None,
        store: None,
        shipped_dir: Some(&shipped),
        shipped_pin: None,
        dev_fallback: None,
        roots: &keys.roots,
    };
    assert_eq!(resolve(&sources).unwrap().recipes.len(), 2);
    let foreign = Keys::new();
    let err = resolve(&RecipeSources {
        roots: &foreign.roots,
        ..sources
    })
    .unwrap_err();
    assert!(matches!(err, BundleError::Signature(_)), "{err}");
}

#[test]
fn invalid_dev_dir_is_an_error_not_a_fallback() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let dev = tmp.path().join("dev");
    write_tree(&dev, "Alpha");
    std::fs::write(dev.join("TIERS.yaml"), "verified: [nope]\n").unwrap();
    let sources = RecipeSources {
        dev_dir: Some(&dev),
        store: None,
        shipped_dir: None,
        shipped_pin: None,
        dev_fallback: None,
        roots: &keys.roots,
    };
    assert!(matches!(resolve(&sources), Err(BundleError::Tiers(_))));
}

#[test]
fn dev_fallback_is_used_only_when_nothing_else_is_available() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let fallback = tmp.path().join("fallback");
    write_tree(&fallback, "Fallback alpha");
    let shipped = tmp.path().join("shipped");
    bundle(&keys, tmp.path(), &shipped, 2, "Shipped alpha");
    let mut sources = RecipeSources {
        dev_dir: None,
        store: None,
        shipped_dir: Some(&shipped),
        shipped_pin: None,
        dev_fallback: Some(&fallback),
        roots: &keys.roots,
    };
    assert!(matches!(
        resolve(&sources).unwrap().source,
        RecipeSource::Shipped { .. }
    ));

    sources.shipped_dir = None;
    let resolved = resolve(&sources).unwrap();
    assert_eq!(resolved.source, RecipeSource::DevDir(fallback.clone()));
    assert!(!resolved.source.is_verified());
    assert_eq!(resolved.created, None);
}

#[test]
fn resolved_bundles_report_their_creation_time() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let shipped = tmp.path().join("shipped");
    bundle(&keys, tmp.path(), &shipped, 1, "Alpha");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let mut sources = RecipeSources {
        dev_dir: None,
        store: Some(&store),
        shipped_dir: Some(&shipped),
        shipped_pin: None,
        dev_fallback: None,
        roots: &keys.roots,
    };
    assert_eq!(resolve(&sources).unwrap().created, Some(NOW));

    Updater::new(DirFetcher::new(&shipped), keys.roots.clone())
        .update(&store, NOW)
        .unwrap();
    sources.shipped_dir = None;
    let resolved = resolve(&sources).unwrap();
    assert_eq!(resolved.source, RecipeSource::Installed(BundleVersion(1)));
    assert_eq!(resolved.created, Some(NOW));
}
