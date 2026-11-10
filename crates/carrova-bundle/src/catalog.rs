//! Parsed recipes with their tiers, from a bundle or a recipe tree.

use std::path::Path;

use carrova_crypto::sign::TrustRoots;
use carrova_recipe::{Recipe, load_str, validate};
use serde::Serialize;

use crate::{
    Asset, BlobRef, BundleError, BundleIndex, Fetcher, RecipeEntry, Result, SignedIndex, SkipCause,
    SkippedRecipe, Tier, VerifyTime,
};

/// A recipe and its trust tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BundledRecipe {
    /// Trust tier.
    pub tier: Tier,
    /// The parsed recipe.
    pub recipe: Recipe,
}

/// Usable recipes plus the ones skipped (unsupported schema, or failing
/// validation).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    /// Parsed recipes, sorted by id.
    pub recipes: Vec<BundledRecipe>,
    /// Recipes left out, with the reason.
    pub skipped: Vec<SkippedRecipe>,
}

impl Catalog {
    /// Every recipe of an unsigned recipe tree (dev override). Fails on any
    /// load error or validation issue; tiers come from its `TIERS.yaml`.
    pub fn from_tree(dir: &Path) -> Result<Self> {
        let (loaded, tiers) = crate::build::load_tiered_tree(dir)?;
        let mut recipes: Vec<BundledRecipe> = loaded
            .into_iter()
            .map(|l| BundledRecipe {
                tier: tiers.tier_of(&l.recipe.id),
                recipe: l.recipe,
            })
            .collect();
        recipes.sort_by(|a, b| a.recipe.id.cmp(&b.recipe.id));
        Ok(Self {
            recipes,
            skipped: Vec::new(),
        })
    }

    /// Parse the supported recipes of `index`, reading each blob with
    /// `read` (which must return the blob's bytes; they are verified here).
    /// A recipe failing [`carrova_recipe::validate`] (the build-time
    /// policy; a signature proves origin, not correctness) is skipped with
    /// its issues.
    pub(crate) fn from_index(
        index: &BundleIndex,
        mut read: impl FnMut(&RecipeEntry) -> Result<Vec<u8>>,
    ) -> Result<Self> {
        let (supported, mut skipped) = index.partition();
        let mut recipes = Vec::with_capacity(supported.len());
        for entry in supported {
            let bundled = recipe_from_blob(entry, &read(entry)?)?;
            let issues = validate(&bundled.recipe);
            if issues.is_empty() {
                recipes.push(bundled);
            } else {
                tracing::warn!(id = %entry.id, issues = issues.len(), "bundled recipe fails validation; skipped");
                skipped.push(SkippedRecipe {
                    id: entry.id.clone(),
                    reason: SkipCause::Invalid {
                        issues: issues.iter().map(ToString::to_string).collect(),
                    },
                });
            }
        }
        Ok(Self { recipes, skipped })
    }
}

/// Verify that `bytes` are `asset` as described by `blob`.
pub(crate) fn check_blob(asset: &Asset, blob: &BlobRef, bytes: &[u8]) -> Result<()> {
    if blob.matches(bytes) {
        Ok(())
    } else {
        Err(BundleError::BlobMismatch {
            name: asset.file_name(),
        })
    }
}

/// Verify and parse one recipe blob, checking it agrees with its entry.
fn recipe_from_blob(entry: &RecipeEntry, bytes: &[u8]) -> Result<BundledRecipe> {
    let asset = entry.asset();
    check_blob(&asset, &entry.blob(), bytes)?;
    let name = asset.file_name();
    let mismatch = |reason: String| BundleError::EntryMismatch {
        name: name.clone(),
        reason,
    };
    let text = std::str::from_utf8(bytes).map_err(|_| mismatch("not UTF-8".into()))?;
    let recipe = load_str(text, Path::new(&name))?;
    if recipe.id != entry.id || recipe.category != entry.category || recipe.schema != entry.schema {
        return Err(mismatch(format!(
            "blob is {}/{} schema {}, index says {}/{} schema {}",
            recipe.category, recipe.id, recipe.schema, entry.category, entry.id, entry.schema
        )));
    }
    Ok(BundledRecipe {
        tier: entry.tier,
        recipe,
    })
}

/// Fetch, verify (signature at `time`, every blob hash) and parse a whole
/// bundle, e.g. one shipped in an app resource dir or a freshly built one.
pub fn open_bundle(
    fetcher: &dyn Fetcher,
    roots: &TrustRoots,
    time: VerifyTime,
) -> Result<(SignedIndex, Catalog)> {
    let signed = fetch_signed(fetcher, roots, time)?;
    fetch_blob(
        fetcher,
        &signed.index().detect_asset(),
        &signed.index().detect,
    )?;
    let catalog = Catalog::from_index(signed.index(), |entry| {
        fetch_blob(fetcher, &entry.asset(), &entry.blob())
    })?;
    Ok((signed, catalog))
}

/// Fetch `index.json` + `index.sig.json` and verify them.
pub(crate) fn fetch_signed(
    fetcher: &dyn Fetcher,
    roots: &TrustRoots,
    time: VerifyTime,
) -> Result<SignedIndex> {
    let index = fetch(fetcher, &Asset::Index, Asset::Index.max_size())?;
    let sig = fetch(fetcher, &Asset::Signature, Asset::Signature.max_size())?;
    SignedIndex::verify(index, sig, roots, time)
}

/// Fetch one blob, capped at its recorded size, and verify it.
pub(crate) fn fetch_blob(fetcher: &dyn Fetcher, asset: &Asset, blob: &BlobRef) -> Result<Vec<u8>> {
    let bytes = fetch(fetcher, asset, blob.size)?;
    check_blob(asset, blob, &bytes)?;
    Ok(bytes)
}

fn fetch(fetcher: &dyn Fetcher, asset: &Asset, limit: u64) -> Result<Vec<u8>> {
    fetcher
        .get(asset, limit)
        .map_err(|source| BundleError::Fetch {
            name: asset.file_name(),
            source,
        })
}
