//! A front-end's candidate recipe locations, resolved with the platform's
//! bundle store: the one way every front-end (CLI, desktop) loads recipes.
//!
//! Each front-end only supplies its own shipped-bundle directory (the
//! CLI's FHS share dir, the desktop app's resource dir) and its
//! [`DevOverride`] policy; the source checkout fallback
//! ([`checkout_recipes_dir`]) and which source wins
//! ([`crate::resolve`]'s documented order) are the same for all.
//!
//! # `CARROVA_RECIPES` ([`RECIPES_ENV`])
//!
//! | Front-end | Policy | Why |
//! |-----------|--------|-----|
//! | CLI | [`DevOverride::Honoured`] | an explicit developer tool (like `--recipes DIR`); warned on every use |
//! | Desktop | [`DevOverride::DebugBuildsOnly`] | nothing in a release app's environment may make it load unsigned recipes |

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use carrova_crypto::sign::TrustRoots;
use carrova_platform::Platform;

use crate::{
    Asset, BundleStore, RecipeSources, ResolvedRecipes, Result, resolve, shipped_index_pin,
};

/// Where a front-end may find recipes, besides the installed bundle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecipeLocations {
    /// Developer override (`--recipes DIR` / [`RECIPES_ENV`]): unsigned.
    pub dev_override: Option<PathBuf>,
    /// Bundle shipped with the app or a packaged install, if present.
    pub shipped_dir: Option<PathBuf>,
    /// Unsigned tree used when nothing else is available (the source
    /// checkout in debug builds).
    pub dev_fallback: Option<PathBuf>,
}

/// Environment variable naming an unsigned developer recipe tree.
pub const RECIPES_ENV: &str = "CARROVA_RECIPES";

/// How a front-end treats [`RECIPES_ENV`] (see the module table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevOverride {
    /// Honoured in every build (the CLI; the caller warns that the
    /// recipes are unverified).
    Honoured,
    /// Honoured only by debug builds (the desktop app); release builds
    /// ignore it.
    DebugBuildsOnly,
}

impl DevOverride {
    /// Whether a build with `debug_build` (`cfg!(debug_assertions)`) reads
    /// [`RECIPES_ENV`] under this policy.
    #[must_use]
    pub const fn honours_env(self, debug_build: bool) -> bool {
        match self {
            Self::Honoured => true,
            Self::DebugBuildsOnly => debug_build,
        }
    }

    /// The developer override: `explicit` (e.g. `--recipes DIR`) first,
    /// else the [`RECIPES_ENV`] value `env` if this build honours it.
    #[must_use]
    pub fn select(self, explicit: Option<PathBuf>, env: Option<OsString>) -> Option<PathBuf> {
        explicit.or_else(|| {
            let dir = env?;
            if self.honours_env(cfg!(debug_assertions)) {
                Some(PathBuf::from(dir))
            } else {
                tracing::info!("{RECIPES_ENV} is ignored by release builds");
                None
            }
        })
    }

    /// [`Self::select`] with [`RECIPES_ENV`] read from this process.
    #[must_use]
    pub fn from_env(self, explicit: Option<PathBuf>) -> Option<PathBuf> {
        self.select(explicit, std::env::var_os(RECIPES_ENV))
    }
}

/// The source checkout's `recipes/` tree, in debug builds only and if it
/// exists: lets `cargo run` work without a bundle. Release builds never
/// fall back to an unsigned tree.
#[must_use]
pub fn checkout_recipes_dir() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../recipes"))
        .canonicalize()
        .ok()
}

impl RecipeLocations {
    /// A front-end's locations: its developer override (from
    /// [`DevOverride::select`]), its shipped bundle directory if it holds
    /// a bundle ([`Self::shipped_bundle`]) and the checkout fallback
    /// ([`checkout_recipes_dir`]).
    #[must_use]
    pub fn new(dev_override: Option<PathBuf>, shipped_dir: Option<PathBuf>) -> Self {
        Self {
            dev_override,
            shipped_dir: shipped_dir.and_then(Self::shipped_bundle),
            dev_fallback: checkout_recipes_dir(),
        }
    }

    /// `dir` if it holds a bundle (its index file), for
    /// [`Self::shipped_dir`]: a missing shipped bundle is skipped, not an
    /// error.
    #[must_use]
    pub fn shipped_bundle(dir: PathBuf) -> Option<PathBuf> {
        dir.join(Asset::INDEX_FILE).is_file().then_some(dir)
    }

    /// The unsigned tree a developer works on: the override, else the
    /// fallback.
    #[must_use]
    pub fn dev_tree(&self) -> Option<&Path> {
        self.dev_override
            .as_deref()
            .or(self.dev_fallback.as_deref())
    }

    /// These locations plus `store` as [`resolve`] input, with this
    /// build's shipped-bundle pin ([`shipped_index_pin`]).
    #[must_use]
    pub fn sources<'a>(
        &'a self,
        store: Option<&'a BundleStore<'a>>,
        roots: &'a TrustRoots,
    ) -> RecipeSources<'a> {
        RecipeSources {
            dev_dir: self.dev_override.as_deref(),
            store,
            shipped_dir: self.shipped_dir.as_deref(),
            shipped_pin: shipped_index_pin(),
            dev_fallback: self.dev_fallback.as_deref(),
            roots,
        }
    }

    /// Resolve the recipes to use with `platform`'s bundle store. A store
    /// that cannot be opened is logged and skipped, like a damaged one.
    /// The caller warns the user when the result is unverified.
    ///
    /// # Errors
    ///
    /// See [`resolve`].
    pub fn resolve(&self, platform: &dyn Platform, roots: &TrustRoots) -> Result<ResolvedRecipes> {
        let store = BundleStore::open(platform, roots.clone())
            .inspect_err(|e| tracing::warn!(error = %e, "recipe bundle store unavailable"))
            .ok();
        resolve(&self.sources(store.as_ref(), roots))
    }
}
