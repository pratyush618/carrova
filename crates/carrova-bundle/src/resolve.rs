//! Which recipes a front-end uses.
//!
//! Order (first available wins):
//!
//! 1. **Dev dir** (`--recipes DIR`): an unsigned recipe tree, loaded and
//!    validated. The caller must warn the user that it is unverified.
//! 2. **Installed bundle**: the verified bundle in the [`BundleStore`]
//!    (signature chain re-verified against the current roots, blob hashes
//!    re-checked on load). An untrusted (revoked) or damaged one is
//!    skipped and reported in [`ResolvedRecipes::rejected_install`].
//! 3. **Shipped bundle**: a built and signed bundle directory in the
//!    app/CLI resources, verified at its signing time
//!    ([`VerifyTime::SignedAt`]) and, in packaged builds, against the
//!    index hash pinned at build time ([`crate::shipped_index_pin`]).
//!
//! Every recipe loaded from a bundle must also pass
//! [`carrova_recipe::validate`]; failing ones are skipped with their
//! issues. A dev dir is strict: any invalid recipe fails the load.
//! 4. **Dev fallback**: an unsigned recipe tree used only when none of the
//!    above is available (e.g. the source checkout's `recipes/` in a debug
//!    build). Reported as [`RecipeSource::DevDir`]: unverified, the caller
//!    must warn like for (1).

use std::path::{Path, PathBuf};

use carrova_crypto::sign::{SignError, TrustRoots};
use carrova_plugin::{Plugin, PluginPackage};

use crate::shipped::open_shipped;
use crate::{
    BlobHash, BundleError, BundleStore, BundleVersion, BundledRecipe, Catalog, DirFetcher, Result,
    SkippedRecipe,
};

/// Where resolved recipes came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeSource {
    /// An unsigned recipe tree given explicitly (developer override).
    DevDir(PathBuf),
    /// The installed, verified bundle.
    Installed(BundleVersion),
    /// The bundle shipped with the app.
    Shipped {
        /// Bundle directory.
        dir: PathBuf,
        /// Its version.
        version: BundleVersion,
    },
}

impl RecipeSource {
    /// Whether the recipes are signature-verified.
    #[must_use]
    pub fn is_verified(&self) -> bool {
        !matches!(self, Self::DevDir(_))
    }
}

/// Why the installed bundle was passed over.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstalledRejected {
    /// Its signature chain no longer verifies against the trusted roots
    /// (e.g. its root was removed from the list: revoked).
    #[error("installed recipe bundle {version} is no longer trusted: {reason}")]
    Untrusted {
        /// Installed version.
        version: BundleVersion,
        /// Why the chain was rejected.
        reason: SignError,
    },
    /// It is damaged or unreadable.
    #[error("installed recipe bundle is unusable: {0}")]
    Unusable(String),
}

impl From<BundleError> for InstalledRejected {
    fn from(err: BundleError) -> Self {
        match err {
            BundleError::InstalledUntrusted { version, source } => Self::Untrusted {
                version,
                reason: source,
            },
            other => Self::Unusable(other.to_string()),
        }
    }
}

/// Candidate sources; `None` skips that step.
#[derive(Debug, Clone, Copy)]
pub struct RecipeSources<'a> {
    /// Developer override directory.
    pub dev_dir: Option<&'a Path>,
    /// Local bundle store.
    pub store: Option<&'a BundleStore<'a>>,
    /// Shipped bundle directory.
    pub shipped_dir: Option<&'a Path>,
    /// Hash its `index.json` must have ([`crate::shipped_index_pin`]);
    /// `None` = signature chain only.
    pub shipped_pin: Option<BlobHash>,
    /// Unsigned tree used when nothing else is available (debug builds).
    pub dev_fallback: Option<&'a Path>,
    /// Roots trusted for the shipped bundle.
    pub roots: &'a TrustRoots,
}

/// Recipes to use and where they came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRecipes {
    /// Chosen source.
    pub source: RecipeSource,
    /// Usable recipes, sorted by id.
    pub recipes: Vec<BundledRecipe>,
    /// Recipes left out (unsupported schema, or failing validation).
    pub skipped: Vec<SkippedRecipe>,
    /// Plugins from the same source (unsigned with a dev dir), not yet
    /// compiled.
    pub plugins: Vec<PluginPackage>,
    /// When the bundle was built (unix seconds); `None` for a dev dir.
    pub created: Option<u64>,
    /// Why an installed bundle was passed over, if one was.
    pub rejected_install: Option<InstalledRejected>,
}

impl ResolvedRecipes {
    fn new(source: RecipeSource, created: Option<u64>, catalog: Catalog) -> Self {
        Self {
            source,
            recipes: catalog.recipes,
            skipped: catalog.skipped,
            plugins: catalog.plugins,
            created,
            rejected_install: None,
        }
    }

    /// Compile [`Self::plugins`] for an engine. One that fails to load
    /// (refused import, broken ABI) is left out and returned as a
    /// [`BundleError::InvalidPlugin`] for the front-end to warn about.
    #[must_use]
    pub fn load_plugins(&self) -> (Vec<Plugin>, Vec<BundleError>) {
        let mut loaded = Vec::new();
        let mut failed = Vec::new();
        for package in &self.plugins {
            let id = package.manifest.id.to_string();
            match Plugin::load(package.clone()) {
                Ok(plugin) => loaded.push(plugin),
                Err(e) => failed.push(BundleError::InvalidPlugin {
                    id,
                    reason: e.to_string(),
                }),
            }
        }
        (loaded, failed)
    }

    /// Recipes of the unsigned tree at `dir`.
    fn dev_dir(dir: &Path) -> Result<Self> {
        // The front-end owns the user-facing warning; this is the log line.
        tracing::info!(dir = %dir.display(), "using unsigned recipes from a dev directory");
        let catalog = Catalog::from_tree(dir)?;
        Ok(Self::new(
            RecipeSource::DevDir(dir.to_path_buf()),
            None,
            catalog,
        ))
    }
}

/// Pick the first available source (see the module docs for the order).
/// A dev dir that fails to load is an error (the user asked for it);
/// an untrusted or damaged installed bundle falls through to the next
/// source, with the reason in [`ResolvedRecipes::rejected_install`].
pub fn resolve(sources: &RecipeSources<'_>) -> Result<ResolvedRecipes> {
    if let Some(dir) = sources.dev_dir {
        return ResolvedRecipes::dev_dir(dir);
    }
    let mut rejected = None;
    if let Some(store) = sources.store {
        match installed(store) {
            Ok(Some(resolved)) => return Ok(resolved),
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(error = %e, "installed recipe bundle passed over");
                rejected = Some(InstalledRejected::from(e));
            }
        }
    }
    let mut resolved = fallback(sources)?;
    resolved.rejected_install = rejected;
    Ok(resolved)
}

/// The sources after the installed bundle: shipped, then dev fallback.
fn fallback(sources: &RecipeSources<'_>) -> Result<ResolvedRecipes> {
    if let Some(dir) = sources.shipped_dir {
        let (signed, catalog) =
            open_shipped(&DirFetcher::new(dir), sources.roots, sources.shipped_pin)?;
        let source = RecipeSource::Shipped {
            dir: dir.to_path_buf(),
            version: signed.version(),
        };
        return Ok(ResolvedRecipes::new(
            source,
            Some(signed.index().created),
            catalog,
        ));
    }
    if let Some(dir) = sources.dev_fallback {
        return ResolvedRecipes::dev_dir(dir);
    }
    Err(BundleError::NoRecipes)
}

fn installed(store: &BundleStore<'_>) -> Result<Option<ResolvedRecipes>> {
    let Some(bundle) = store.installed()? else {
        return Ok(None);
    };
    let catalog = store.catalog(&bundle.index)?;
    Ok(Some(ResolvedRecipes::new(
        RecipeSource::Installed(bundle.version()),
        Some(bundle.index.created),
        catalog,
    )))
}
