//! `carrova recipes` internals: refused versions, `reset`, and the warning
//! for a passed-over installed bundle. Bundles are signed by test roots.

use anyhow::Result;
use carrova_bundle::{BundleStore, BundleVersion, RecipeSource, UpdateOutcome};

use super::{Fixture, Signer, resolve_recipes};
use crate::recipes::RecipeLocations;
use crate::recipes::source::rejected_warning;
use crate::recipes::update::{ResetResult, reset_store};
use crate::render::recipes::{reset_outcome, update_outcome};

fn shipped(f: &Fixture) -> RecipeLocations {
    RecipeLocations {
        shipped_dir: Some(f.path("shipped")),
        ..RecipeLocations::default()
    }
}

#[test]
fn a_rolled_back_bundle_is_reported_as_not_reinstalled() -> Result<()> {
    let f = Fixture::new()?;
    f.install(4)?;
    let v5 = f.release(5)?;
    f.update(&v5)?;
    f.store()?.rollback()?;
    let outcome = f.update(&v5)?;
    assert!(matches!(outcome, UpdateOutcome::Refused(BundleVersion(5))));
    let mut text = Vec::new();
    update_outcome(&mut text, &outcome)?;
    assert_eq!(
        String::from_utf8(text)?,
        "Recipe bundle v5 was rolled back earlier and is not installed again; \
         keeping the current recipes.\n"
    );
    Ok(())
}

#[test]
fn reset_asks_first_and_then_falls_back_to_the_shipped_bundle() -> Result<()> {
    let f = Fixture::new()?;
    f.install(5)?;
    let store = f.store()?;
    let mut asked = String::new();
    let declined = reset_store(&store, |q| {
        asked = q.to_owned();
        Ok(false)
    })?;
    assert_eq!(declined, ResetResult::Declined);
    assert!(asked.contains("shipped with Carrova"), "{asked}");
    assert_eq!(
        f.resolve(&shipped(&f))?,
        RecipeSource::Installed(BundleVersion(5))
    );

    let reset = reset_store(&store, |_| Ok(true))?;
    assert_eq!(
        reset,
        ResetResult::Reset {
            removed: Some(BundleVersion(5))
        }
    );
    assert!(matches!(
        f.resolve(&shipped(&f))?,
        RecipeSource::Shipped {
            version: BundleVersion(3),
            ..
        }
    ));
    let mut text = Vec::new();
    reset_outcome(&mut text, Some(BundleVersion(5)))?;
    reset_outcome(&mut text, None)?;
    assert_eq!(
        String::from_utf8(text)?,
        "Removed recipe bundle v5; Carrova now uses the recipes it shipped with.\n\
         No recipe bundle was installed; nothing to reset.\n"
    );
    Ok(())
}

#[test]
fn a_revoked_installed_bundle_is_passed_over_with_a_reset_hint() -> Result<()> {
    let f = Fixture::new()?;
    f.install(5)?;
    let other = Signer::new()?;
    let store = BundleStore::open(&f.platform, other.roots.clone())?;
    // The shipped bundle (signed by the fixture's root) is untrusted too,
    // so trust only `other` and fall through to a dev tree.
    let locations = RecipeLocations {
        dev_fallback: Some(f.path("checkout")),
        ..RecipeLocations::default()
    };
    let resolved = resolve_recipes(&locations.sources(Some(&store), &other.roots))?;
    assert_eq!(resolved.source, RecipeSource::DevDir(f.path("checkout")));
    let warning = rejected_warning(&resolved).unwrap_or_default();
    assert!(warning.contains("v5 is no longer trusted"), "{warning}");
    assert!(warning.contains("carrova recipes reset"), "{warning}");
    Ok(())
}
