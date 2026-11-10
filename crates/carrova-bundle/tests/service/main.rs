//! The recipes service every front-end wraps: effective tiers, the
//! "recipes in use" summary, the catalogue, the update attempt (404 →
//! retry later), the developer-override policy and the clock.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../bundle/support.rs"]
pub mod support;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use carrova_bundle::{
    Asset, BundleVersion, DevOverride, DirFetcher, EffectiveTier, FetchError, Fetcher,
    RecipeLocations, RecipeSource, ResolvedRecipes, SkipCause, TierCounts, UpdateAttempt,
    checkout_recipes_dir, now_unix, update_with,
};

use support::{Keys, NOW, bundle, platform, store, write_tree};

/// `alpha` (verified in TIERS.yaml) and `beta` (community) from an unsigned tree.
fn resolve_tree(tmp: &Path) -> ResolvedRecipes {
    let tree = tmp.join("tree");
    write_tree(&tree, "Alpha");
    let locations = RecipeLocations {
        dev_override: Some(tree),
        ..RecipeLocations::default()
    };
    locations
        .resolve(&platform(&tmp.join("home")), &Keys::new().roots)
        .unwrap()
}

/// The same recipes from an installed, signed bundle.
fn resolve_installed(tmp: &Path) -> ResolvedRecipes {
    let keys = Keys::new();
    let out = tmp.join("out");
    bundle(&keys, tmp, &out, 1, "Alpha");
    let p = platform(&tmp.join("home"));
    let store = store(&p, &keys.roots);
    update_with(DirFetcher::new(&out), keys.roots.clone(), &store, NOW).unwrap();
    RecipeLocations::default().resolve(&p, &keys.roots).unwrap()
}

fn tiers(resolved: &ResolvedRecipes) -> Vec<(String, EffectiveTier)> {
    resolved
        .recipes
        .iter()
        .map(|r| (r.recipe.id.clone(), resolved.effective_tier(r)))
        .collect()
}

#[test]
fn recipes_from_an_unsigned_tree_are_unverified_whatever_tiers_yaml_says() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = resolve_tree(tmp.path());
    assert_eq!(
        tiers(&resolved),
        [
            ("alpha".to_owned(), EffectiveTier::Unverified),
            ("beta".to_owned(), EffectiveTier::Unverified),
        ]
    );
}

#[test]
fn recipes_from_a_signed_bundle_keep_their_tier() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = resolve_installed(tmp.path());
    assert_eq!(
        tiers(&resolved),
        [
            ("alpha".to_owned(), EffectiveTier::Verified),
            ("beta".to_owned(), EffectiveTier::Community),
        ]
    );
}

#[test]
fn recipes_in_use_count_effective_tiers() {
    let tmp = tempfile::tempdir().unwrap();
    let dev = resolve_tree(&tmp.path().join("dev")).in_use();
    assert!(!dev.verified);
    assert_eq!(dev.created, None);
    assert_eq!(
        dev.counts,
        TierCounts {
            total: 2,
            verified: 0,
            community: 0,
            unverified: 2,
        }
    );

    let installed = resolve_installed(&tmp.path().join("installed")).in_use();
    assert!(installed.verified);
    assert_eq!(installed.source, RecipeSource::Installed(BundleVersion(1)));
    assert_eq!(installed.created, Some(NOW));
    assert_eq!(
        installed.counts,
        TierCounts {
            total: 2,
            verified: 1,
            community: 1,
            unverified: 0,
        }
    );
}

#[test]
fn recipes_in_use_carry_skipped_recipes_and_their_reasons() {
    let tmp = tempfile::tempdir().unwrap();
    let mut resolved = resolve_tree(tmp.path());
    resolved.skipped.push(carrova_bundle::SkippedRecipe {
        id: "future".into(),
        reason: SkipCause::UnsupportedSchema { schema: 9 },
    });
    let in_use = resolved.in_use();
    assert_eq!(in_use.skipped, resolved.skipped);
}

#[test]
fn the_catalogue_lists_every_recipe_with_its_effective_tier_and_detection() {
    let tmp = tempfile::tempdir().unwrap();
    let resolved = resolve_tree(tmp.path());
    let p = platform(&tmp.path().join("home"));
    let apps: Vec<_> = resolved
        .catalog(&p)
        .into_iter()
        .map(|app| (app.recipe.id.clone(), app.tier, app.detected))
        .collect();
    assert_eq!(
        apps,
        [
            ("alpha".to_owned(), EffectiveTier::Unverified, false),
            ("beta".to_owned(), EffectiveTier::Unverified, false),
        ]
    );
}

/// A release feed that has nothing published (every asset is a 404).
#[derive(Debug)]
struct Unpublished;

impl Fetcher for Unpublished {
    fn get(&self, _: &Asset, _: u64) -> Result<Vec<u8>, FetchError> {
        Err(FetchError::NotFound)
    }
}

#[test]
fn a_missing_bundle_means_retry_later_not_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let p = platform(&tmp.path().join("home"));
    let attempt = update_with(
        Unpublished,
        keys.roots.clone(),
        &store(&p, &keys.roots),
        NOW,
    );
    assert!(
        matches!(attempt, Ok(UpdateAttempt::RetryLater)),
        "{attempt:?}"
    );
}

#[test]
fn an_update_reports_the_version_it_installed_and_none_when_current() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 4, "Alpha");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let update = || update_with(DirFetcher::new(&out), keys.roots.clone(), &store, NOW).unwrap();

    let UpdateAttempt::Done(first) = update() else {
        panic!("expected an install");
    };
    assert_eq!(first.installed(), Some(BundleVersion(4)));
    let UpdateAttempt::Done(again) = update() else {
        panic!("expected up to date");
    };
    assert_eq!(again.installed(), None);
}

#[test]
fn the_cli_honours_carrova_recipes_in_every_build() {
    assert!(DevOverride::Honoured.honours_env(true));
    assert!(DevOverride::Honoured.honours_env(false));
    assert_eq!(
        DevOverride::Honoured.select(None, Some(OsString::from("/tmp/tree"))),
        Some(PathBuf::from("/tmp/tree"))
    );
}

#[test]
fn the_desktop_app_honours_carrova_recipes_only_in_debug_builds() {
    assert!(DevOverride::DebugBuildsOnly.honours_env(true));
    assert!(!DevOverride::DebugBuildsOnly.honours_env(false));
    assert_eq!(
        DevOverride::DebugBuildsOnly
            .select(None, Some(OsString::from("/tmp/tree")))
            .is_some(),
        cfg!(debug_assertions)
    );
}

#[test]
fn an_explicit_directory_wins_over_the_environment() {
    for policy in [DevOverride::Honoured, DevOverride::DebugBuildsOnly] {
        assert_eq!(
            policy.select(Some(PathBuf::from("/flag")), Some(OsString::from("/env"))),
            Some(PathBuf::from("/flag"))
        );
        assert_eq!(policy.select(None, None), None);
    }
}

#[test]
fn debug_builds_find_the_checkout_recipe_tree() {
    let tree = checkout_recipes_dir();
    assert_eq!(tree.is_some(), cfg!(debug_assertions));
    if let Some(tree) = tree {
        assert!(tree.ends_with("recipes") && tree.join("TIERS.yaml").is_file());
    }
}

#[test]
fn front_end_locations_use_the_checkout_as_fallback_and_skip_a_missing_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let locations = RecipeLocations::new(None, Some(tmp.path().to_path_buf()));
    assert_eq!(locations.shipped_dir, None);
    assert_eq!(locations.dev_fallback, checkout_recipes_dir());
}

#[test]
fn the_clock_reads_unix_seconds() {
    assert!(now_unix().unwrap() > 1_700_000_000);
}
