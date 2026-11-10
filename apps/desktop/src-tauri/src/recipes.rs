//! Recipes for the desktop app: where to look ([`app_locations`]), the
//! resolved set every new session uses ([`RecipeService::current`]), and
//! the catalogue / status / update / reset views.
//!
//! Everything about recipes is the shared recipes service in
//! `carrova-bundle` (source order, `CARROVA_RECIPES` policy, effective
//! tiers, status, catalogue, update and its retry policy), used by the CLI
//! too; this module only supplies the app's paths and maps the results to
//! IPC types.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use carrova_bundle::{
    BundleStore, DevOverride, EffectiveTier, HttpConfig, KnownApp, RecipeLocations, RecipeSource,
    RecipesInUse, ResolvedRecipes, UpdateAttempt, release_roots,
};
use carrova_core::{Plugin, Recipe};
use carrova_platform::Platform;
use carrova_view::ipc::{
    CatalogEntry, CatalogItem, RecipeOrigin, RecipeTier, RecipesCheck, RecipesStatus, RecipesUpdate,
};

use crate::command_error::CommandError;

/// The app's candidate recipe locations: `CARROVA_RECIPES` (debug builds
/// only, [`DevOverride::DebugBuildsOnly`]), the bundle shipped in the
/// app's `resource_dir` (`recipes/`, see `tauri.release.conf.json`) and,
/// in debug builds, the source checkout's unsigned tree.
#[must_use]
pub fn app_locations(resource_dir: Option<&Path>) -> RecipeLocations {
    RecipeLocations::new(
        DevOverride::DebugBuildsOnly.from_env(None),
        resource_dir.map(|dir| dir.join("recipes")),
    )
}

/// Resolved recipes plus the plain lists sessions are built with.
#[derive(Debug)]
pub struct LoadedRecipes {
    /// Source, tiers and skipped recipes.
    pub resolved: ResolvedRecipes,
    /// The recipes, shared by every engine built from them.
    pub recipes: Arc<[Recipe]>,
    /// The compiled plugins from the same source (one that fails to load
    /// is logged and left out).
    pub plugins: Arc<[Plugin]>,
}

/// Resolves recipes once and keeps them until an update installs new ones.
#[derive(Debug)]
pub struct RecipeService {
    platform: Arc<dyn Platform>,
    locations: RecipeLocations,
    loaded: Mutex<Option<Arc<LoadedRecipes>>>,
}

impl RecipeService {
    /// Recipes from `locations` on `platform` (resolved on first use).
    pub fn new(platform: Arc<dyn Platform>, locations: RecipeLocations) -> Self {
        Self {
            platform,
            locations,
            loaded: Mutex::new(None),
        }
    }

    /// The recipes in use, resolving them if needed (`carrova-bundle` logs
    /// an unverified source or a passed-over bundle). Blocking.
    ///
    /// # Errors
    ///
    /// No source is available or the chosen one is invalid.
    pub fn current(&self) -> Result<Arc<LoadedRecipes>, CommandError> {
        let mut loaded = self.loaded.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(current) = &*loaded {
            return Ok(Arc::clone(current));
        }
        let resolved = self
            .locations
            .resolve(self.platform.as_ref(), &release_roots()?)?;
        let recipes = resolved.recipes.iter().map(|b| b.recipe.clone()).collect();
        let (plugins, failed) = resolved.load_plugins();
        for e in failed {
            tracing::warn!(error = %e, "plugin left out");
        }
        let current = Arc::new(LoadedRecipes {
            resolved,
            recipes,
            plugins: plugins.into(),
        });
        *loaded = Some(Arc::clone(&current));
        Ok(current)
    }

    /// Which recipes are in use.
    ///
    /// # Errors
    ///
    /// See [`Self::current`].
    pub fn status(&self) -> Result<RecipesStatus, CommandError> {
        Ok(status(self.current()?.resolved.in_use()))
    }

    /// Every known app, with detection on this machine.
    ///
    /// # Errors
    ///
    /// See [`Self::current`].
    pub fn catalog(&self) -> Result<Vec<CatalogEntry>, CommandError> {
        let current = self.current()?;
        Ok(current
            .resolved
            .catalog(self.platform.as_ref())
            .into_iter()
            .map(catalog_entry)
            .collect())
    }

    /// Check the release feed for a newer verified bundle and install it;
    /// new sessions then use it. Blocking.
    ///
    /// # Errors
    ///
    /// Download, verification or install failure.
    pub fn update(&self) -> Result<RecipesUpdate, CommandError> {
        let attempt = carrova_bundle::update(
            self.platform.as_ref(),
            release_roots()?,
            &HttpConfig::new(carrova_bundle::DEFAULT_BASE_URL),
        )?;
        Ok(match attempt {
            UpdateAttempt::RetryLater => RecipesUpdate::RetryLater,
            UpdateAttempt::Done(outcome) => match outcome.installed() {
                Some(version) => {
                    self.forget();
                    RecipesUpdate::Updated { version: version.0 }
                }
                None => RecipesUpdate::UpToDate,
            },
        })
    }

    /// Whether a newer bundle is offered; installs nothing. Blocking.
    ///
    /// # Errors
    ///
    /// Download or verification failure.
    pub fn check(&self) -> Result<RecipesCheck, CommandError> {
        let attempt = carrova_bundle::check(
            self.platform.as_ref(),
            release_roots()?,
            &HttpConfig::new(carrova_bundle::DEFAULT_BASE_URL),
        )?;
        Ok(attempt.into())
    }

    /// Remove the installed bundle so the shipped recipes are used again,
    /// and report the recipes now in use. Blocking.
    ///
    /// # Errors
    ///
    /// The store cannot be changed, or no recipes are available after it.
    pub fn reset(&self) -> Result<RecipesStatus, CommandError> {
        let store = BundleStore::open(self.platform.as_ref(), release_roots()?)?;
        store.reset()?;
        self.forget();
        self.status()
    }

    /// Drop the resolved recipes: the next session resolves again.
    fn forget(&self) {
        *self.loaded.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
}

/// [`RecipesInUse`] as the IPC status. Counts are by effective tier: an
/// unsigned dev tree counts as `unverified` only.
fn status(in_use: RecipesInUse) -> RecipesStatus {
    let origin = match in_use.source {
        RecipeSource::DevDir(_) => RecipeOrigin::DevDir,
        RecipeSource::Installed(version) => RecipeOrigin::Installed { version: version.0 },
        RecipeSource::Shipped { version, .. } => RecipeOrigin::Shipped { version: version.0 },
    };
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    RecipesStatus {
        origin,
        created: in_use.created,
        verified: count(in_use.counts.verified),
        community: count(in_use.counts.community),
        unverified: count(in_use.counts.unverified),
        skipped: in_use.skipped.into_iter().map(|s| s.id).collect(),
    }
}

/// [`EffectiveTier`] as the IPC tier.
fn tier(tier: EffectiveTier) -> RecipeTier {
    match tier {
        EffectiveTier::Verified => RecipeTier::Verified,
        EffectiveTier::Community => RecipeTier::Community,
        EffectiveTier::Unverified => RecipeTier::Unverified,
    }
}

/// [`KnownApp`] as the IPC catalogue entry.
fn catalog_entry(
    KnownApp {
        recipe,
        tier: effective,
        detected,
    }: KnownApp<'_>,
) -> CatalogEntry {
    CatalogEntry {
        id: recipe.id.clone(),
        name: recipe.name.clone(),
        category: recipe.category.clone(),
        description: recipe.description.clone().unwrap_or_default(),
        tier: tier(effective),
        detected,
        items: recipe
            .items
            .iter()
            .map(|item| CatalogItem {
                id: item.id.clone(),
                label: item.label.clone(),
                default: item.default_selected(),
                sensitive: item.flags.sensitive,
                unmovable: !item.is_movable(),
            })
            .collect(),
    }
}

#[cfg(test)]
#[path = "../tests/recipes/unit.rs"]
mod tests;
