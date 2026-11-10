//! The bundle index (`index.json`) and the policies applied to it.

use std::collections::HashSet;
use std::fmt;

use carrova_recipe::{SUPPORTED_SCHEMA, is_kebab};
use serde::{Deserialize, Serialize};

use crate::{Asset, BlobHash, BlobKind, BlobRef, BundleError, PluginEntry, Result};

/// Bundle format this crate reads and writes (2: per-OS detect index;
/// 3: plugins).
pub const FORMAT: u32 = 3;

/// Monotonic bundle version; each release must be strictly greater.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BundleVersion(pub u64);

impl fmt::Display for BundleVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

/// Trust tier of a recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Reviewed by maintainers (listed in `recipes/TIERS.yaml`).
    Verified,
    /// Contributed; not yet verified.
    Community,
}

/// One recipe in the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeEntry {
    /// Recipe id (kebab-case, unique in the index).
    pub id: String,
    /// Recipe category.
    pub category: String,
    /// Trust tier.
    pub tier: Tier,
    /// Recipe schema version.
    pub schema: u32,
    /// BLAKE3 of the recipe file.
    pub hash: BlobHash,
    /// Size of the recipe file in bytes.
    pub size: u64,
}

impl RecipeEntry {
    /// Hash and size of the recipe blob.
    #[must_use]
    pub fn blob(&self) -> BlobRef {
        BlobRef {
            hash: self.hash,
            size: self.size,
        }
    }

    /// The recipe blob's asset name.
    #[must_use]
    pub fn asset(&self) -> Asset {
        Asset::Blob(BlobKind::Recipe, self.hash)
    }

    /// Whether this engine can read the recipe's schema.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        SUPPORTED_SCHEMA.contains(&self.schema)
    }
}

/// A recipe left out of a bundle's usable recipes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedRecipe {
    /// Recipe id.
    pub id: String,
    /// Why it was left out.
    pub reason: SkipCause,
}

/// Why a bundled recipe was left out. [`fmt::Display`] gives the
/// user-facing reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SkipCause {
    /// Its schema is outside [`carrova_recipe::SUPPORTED_SCHEMA`].
    UnsupportedSchema {
        /// The recipe's schema version.
        schema: u32,
    },
    /// It parses but fails [`carrova_recipe::validate`], the policy every
    /// bundle build enforces.
    Invalid {
        /// One line per validation issue.
        issues: Vec<String>,
    },
}

impl fmt::Display for SkipCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema { schema } => write!(
                f,
                "recipe schema {schema} is not supported by this Carrova (supports {}-{})",
                SUPPORTED_SCHEMA.start(),
                SUPPORTED_SCHEMA.end()
            ),
            Self::Invalid { issues } => {
                write!(f, "recipe fails validation: {}", issues.join("; "))
            }
        }
    }
}

/// Contents of `index.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleIndex {
    /// Bundle format ([`FORMAT`]).
    pub format: u32,
    /// Monotonic release version.
    pub version: BundleVersion,
    /// Creation time (unix seconds).
    pub created: u64,
    /// Recipes, sorted by id.
    pub recipes: Vec<RecipeEntry>,
    /// The precompiled detection index blob.
    pub detect: BlobRef,
    /// Plugins, sorted by id.
    pub plugins: Vec<PluginEntry>,
}

impl BundleIndex {
    /// Parse `index.json` bytes and [`Self::validate`] them.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        // The format is checked before the shape so a future format gets
        // a clear error instead of an "unknown field" one.
        #[derive(Deserialize)]
        struct Probe {
            format: u32,
        }
        let probe: Probe = serde_json::from_slice(bytes).map_err(BundleError::json("index"))?;
        check_format(probe.format)?;
        let index: Self = serde_json::from_slice(bytes).map_err(BundleError::json("index"))?;
        index.validate()?;
        Ok(index)
    }

    /// Structural checks a signature cannot replace: format, unique
    /// kebab-case ids, blob sizes within limits.
    pub fn validate(&self) -> Result<()> {
        check_format(self.format)?;
        let invalid = |m: String| Err(BundleError::InvalidIndex(m));
        if self.detect.size > BlobKind::Detect.max_size() {
            return invalid(format!("detect index is {} bytes", self.detect.size));
        }
        let mut seen = HashSet::new();
        for entry in &self.recipes {
            if !is_kebab(&entry.id) {
                return invalid(format!("recipe id `{}` is not kebab-case", entry.id));
            }
            if !seen.insert(entry.id.as_str()) {
                return invalid(format!("duplicate recipe id `{}`", entry.id));
            }
            if entry.size > BlobKind::Recipe.max_size() {
                return invalid(format!("recipe `{}` is {} bytes", entry.id, entry.size));
            }
        }
        let mut seen = HashSet::new();
        for entry in &self.plugins {
            if !seen.insert(&entry.id) {
                return invalid(format!("duplicate plugin id `{}`", entry.id));
            }
            if let Some((asset, blob)) = entry
                .blobs()
                .into_iter()
                .find(|(asset, blob)| blob.size > asset.max_size())
            {
                return invalid(format!(
                    "plugin blob {} is {} bytes",
                    asset.file_name(),
                    blob.size
                ));
            }
        }
        Ok(())
    }

    /// The schema filter, the one place it lives: entries this engine can
    /// read, and the ones it skips.
    #[must_use]
    pub fn partition(&self) -> (Vec<&RecipeEntry>, Vec<SkippedRecipe>) {
        let (supported, skipped): (Vec<&RecipeEntry>, Vec<&RecipeEntry>) =
            self.recipes.iter().partition(|e| e.is_supported());
        let skipped = skipped
            .into_iter()
            .map(|e| SkippedRecipe {
                id: e.id.clone(),
                reason: SkipCause::UnsupportedSchema { schema: e.schema },
            })
            .collect();
        (supported, skipped)
    }

    /// The detect index blob's asset name.
    #[must_use]
    pub fn detect_asset(&self) -> Asset {
        Asset::Blob(BlobKind::Detect, self.detect.hash)
    }

    /// Every blob a client needs: the detect index, each supported
    /// recipe and every plugin's two blobs.
    #[must_use]
    pub fn required_blobs(&self) -> Vec<(Asset, BlobRef)> {
        let (supported, _) = self.partition();
        std::iter::once((self.detect_asset(), self.detect))
            .chain(supported.into_iter().map(|e| (e.asset(), e.blob())))
            .chain(self.plugins.iter().flat_map(PluginEntry::blobs))
            .collect()
    }
}

fn check_format(found: u32) -> Result<()> {
    if found == FORMAT {
        Ok(())
    } else {
        Err(BundleError::UnsupportedFormat {
            found,
            supported: FORMAT,
        })
    }
}
