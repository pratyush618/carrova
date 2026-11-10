//! Where the CLI finds recipes.
//!
//! Which source wins is decided by [`carrova_bundle::resolve`] through
//! [`RecipeLocations`], the policy shared with the desktop app. This module
//! only supplies the CLI's candidate locations ([`for_this_exe`]) and tells
//! the user when recipes are unverified or a bundle was passed over:
//!
//! 1. `--recipes DIR` / `CARROVA_RECIPES`: developer override, unsigned,
//!    honoured in every build ([`DevOverride::Honoured`]: the CLI is an
//!    explicit developer tool) and warned about on every use.
//! 2. The verified bundle installed by `carrova recipes update`.
//! 3. The bundle shipped with a packaged install
//!    ([`shipped_dir_for`]: `<exe dir>/../share/carrova/recipes`).
//! 4. Debug builds only: the source checkout's `recipes/` (unsigned), so
//!    `cargo run` works without a bundle. Release builds without a bundle
//!    fail and point at `carrova recipes update`.

use std::path::{Path, PathBuf};

use anyhow::Result;
use carrova_bundle::{DevOverride, RecipeLocations, RecipeSource, ResolvedRecipes, SkippedRecipe};
use carrova_core::Recipe;

use super::error::{Action, explain};
use super::seams::trusted_roots;
use crate::render::text::safe;

/// The candidate recipe locations of this executable: `--recipes DIR`
/// (`explicit`), else `CARROVA_RECIPES`, as the developer override.
#[must_use]
pub fn for_this_exe(explicit: Option<PathBuf>) -> RecipeLocations {
    RecipeLocations::new(
        DevOverride::Honoured.from_env(explicit),
        std::env::current_exe()
            .ok()
            .and_then(|exe| shipped_dir_for(&exe)),
    )
}

/// Loading recipes the CLI way: errors explained for the terminal,
/// warnings on stderr.
pub trait LoadRecipes {
    /// Resolve the recipes to use, warning on stderr when they are
    /// unverified or an installed bundle was passed over.
    fn load(&self) -> Result<ResolvedRecipes>;

    /// [`Self::load`] for commands that only need the recipes: skipped
    /// recipes are reported on stderr.
    fn recipes(&self) -> Result<Vec<Recipe>>;
}

impl LoadRecipes for RecipeLocations {
    fn load(&self) -> Result<ResolvedRecipes> {
        let platform = carrova_platform::current();
        let roots = trusted_roots().map_err(|e| explain(e, Action::Load))?;
        let resolved = self
            .resolve(platform.as_ref(), &roots)
            .map_err(|e| explain(e, Action::Load))?;
        for warning in [
            rejected_warning(&resolved),
            unverified_warning(&resolved.source),
        ]
        .into_iter()
        .flatten()
        {
            eprintln!("{warning}");
        }
        Ok(resolved)
    }

    fn recipes(&self) -> Result<Vec<Recipe>> {
        let resolved = self.load()?;
        warn_skipped(&resolved.skipped);
        Ok(resolved.recipes.into_iter().map(|b| b.recipe).collect())
    }
}

/// The shipped bundle of a packaged install: `<prefix>/bin/carrova` →
/// `<prefix>/share/carrova/recipes` (FHS layout of `.deb` and tarballs).
#[must_use]
pub fn shipped_dir_for(exe: &Path) -> Option<PathBuf> {
    let prefix = exe.parent()?.parent()?;
    Some(prefix.join("share").join("carrova").join("recipes"))
}

/// The stderr warning for an unverified source, `None` for a verified one.
#[must_use]
pub fn unverified_warning(source: &RecipeSource) -> Option<String> {
    match source {
        RecipeSource::DevDir(dir) => Some(format!(
            "warning: using unverified recipes from {} (developer recipe tree, \
             not signature-checked)",
            safe(&dir.display().to_string())
        )),
        RecipeSource::Installed(_) | RecipeSource::Shipped { .. } => None,
    }
}

/// The stderr warning for an installed bundle that was passed over.
#[must_use]
pub fn rejected_warning(resolved: &ResolvedRecipes) -> Option<String> {
    resolved.rejected_install.as_ref().map(|why| {
        format!(
            "warning: {}; using other recipes instead. Run `carrova recipes reset` \
             to remove it",
            safe(&why.to_string())
        )
    })
}

/// One stderr warning per recipe left out.
pub fn warn_skipped(skipped: &[SkippedRecipe]) {
    for s in skipped {
        eprintln!(
            "warning: skipping recipe {}: {}",
            safe(&s.id),
            safe(&s.reason.to_string())
        );
    }
}
