//! Carrova recipe bundles: a signed, content-addressed snapshot of the
//! recipe tree that clients fetch from GitHub Releases (`recipes-vN`
//! tags) and install without an app release.
//!
//! # Bundle format (`format: 3`)
//!
//! A bundle is a flat set of files (one release, every asset at the top
//! level):
//!
//! | File | Content |
//! |------|---------|
//! | `index.json` | [`BundleIndex`]. Its **raw bytes** are what is signed. |
//! | `index.sig.json` | [`IndexSignature`]: `{ cert, sig }`, the signer's [`SignedKeyCert`] and its signature over `index.json` under [`SignContext::BundleIndex`]. |
//! | `<blake3>.yaml` | One recipe file, named by the lowercase-hex BLAKE3 of its bytes ([`BlobHash`]). |
//! | `<blake3>.json` | [`DetectIndex`]: the per-OS detect rules of every recipe, so detection needs no full parse. Referenced by [`BundleIndex::detect`]. |
//! | `<blake3>.plugin.yaml` | A plugin's manifest ([`carrova_plugin::PluginManifest`]). |
//! | `<blake3>.wasm` | A plugin's WebAssembly module. Both referenced by a [`PluginEntry`] in [`BundleIndex::plugins`]. |
//!
//! [`BundleIndex`] carries a monotonic [`BundleVersion`] (rollback
//! protection), the creation time, and per recipe its id, category,
//! [`Tier`] (from `recipes/TIERS.yaml`, see [`TierPolicy`]), schema
//! version, hash and size, and per plugin its id, version and two blobs
//! (built from `plugins/<id>/` next to the recipe tree, [`plugins_dir`];
//! plugins are core-maintainer reviewed, no tiers). Every name derived
//! from the index goes through [`Asset`], whose hashes are validated as exactly 64 lowercase hex
//! digits, so the index can never name a path.
//!
//! # Trust
//!
//! The signer key is certified by an offline root ([`carrova_crypto::sign`]);
//! clients trust [`release_roots`] (`keys/bundle-roots.json`, embedded at
//! build time). Remote bundles are verified at the current time. Installed
//! bundles are re-verified on every load against the *current* roots at
//! their signing time ([`VerifyTime::SignedAt`]), so removing a root
//! revokes every bundle it signed. A bundle shipped inside the app is
//! verified at its signing time too (the app may outlive the short-lived
//! signer certificate) and, in packaged builds, must match the index hash
//! pinned at build time ([`shipped_index_pin`]).
//!
//! Signatures prove origin, not correctness: every recipe loaded from a
//! bundle must also pass [`carrova_recipe::validate`] or is skipped
//! ([`SkipCause::Invalid`]). Updates follow one admission policy
//! ([`MAX_VERSION_JUMP`], monotonic `created`, rolled-back versions never
//! re-offered), so a leaked signer key cannot lock clients out of later
//! releases.
//!
//! # Flow
//!
//! * Build side ([`build_bundle`], [`sign_bundle`]) is used by `cargo xtask
//!   bundle`.
//! * Client side: [`HttpFetcher::latest`] follows the release pointer
//!   ([`LATEST_POINTER`]) to the immutable `recipes-vN` release,
//!   [`Updater::check`] fetches and verifies the index,
//!   [`Updater::fetch`] downloads only missing blobs into the
//!   [`BundleStore`], [`BundleStore::install`] switches atomically and keeps
//!   the previous version for [`BundleStore::rollback`];
//!   [`BundleStore::reset`] returns to the shipped bundle. Every store
//!   change holds the store lock.
//! * Front-ends describe their [`RecipeLocations`] (developer override
//!   policy [`DevOverride`]) and call [`RecipeLocations::resolve`]
//!   ([`resolve`]) to pick a [`RecipeSource`]; what they show and how they
//!   update is the shared recipes service ([`ResolvedRecipes::in_use`],
//!   [`ResolvedRecipes::catalog`], [`ResolvedRecipes::effective_tier`],
//!   [`update`]).

mod build;
mod catalog;
mod clock;
mod detect;
mod error;
mod fetch;
mod hash;
mod index;
mod locations;
mod plugins;
mod policy;
mod resolve;
mod roots;
mod service;
mod shipped;
mod signed;
mod store;
mod tiers;
mod update;

pub use build::{BuildReport, build_bundle, load_checked_tree, sign_bundle};
pub use carrova_crypto::sign::{SignContext, SignedKeyCert, TrustRoots};
pub use catalog::{BundledRecipe, Catalog, open_bundle};
pub use clock::now_unix;
pub use detect::DetectIndex;
pub use error::{BundleError, FetchError, Result};
pub use fetch::{
    DEFAULT_BASE_URL, DirFetcher, Fetcher, HttpConfig, HttpFetcher, LATEST_POINTER, LatestPointer,
    POINTER_MAX_SIZE,
};
pub use hash::{Asset, BlobHash, BlobKind, BlobRef};
pub use index::{BundleIndex, BundleVersion, FORMAT, RecipeEntry, SkipCause, SkippedRecipe, Tier};
pub use locations::{DevOverride, RECIPES_ENV, RecipeLocations, checkout_recipes_dir};
pub use plugins::{PLUGIN_MANIFEST_FILE, PLUGIN_MODULE_FILE, PluginEntry, plugins_dir};
pub use policy::MAX_VERSION_JUMP;
pub use resolve::{InstalledRejected, RecipeSource, RecipeSources, ResolvedRecipes, resolve};
pub use roots::{parse_roots, release_roots};
pub use service::{
    CheckAttempt, EffectiveTier, KnownApp, RecipesInUse, TierCounts, UpdateAttempt, check, update,
    update_with,
};
pub use shipped::{SHIPPED_INDEX_HASH_ENV, shipped_index_pin};
pub use signed::{IndexSignature, SignedIndex, VerifyTime};
pub use store::{BundleStore, InstallOutcome, InstalledBundle};
pub use tiers::{TIERS_FILE, TierPolicy};
pub use update::{FetchStats, PendingUpdate, UpdateCheck, UpdateOutcome, Updater};
