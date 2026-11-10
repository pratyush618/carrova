//! Precompiled detection index (`<hash>.json`).

use std::collections::BTreeMap;

use carrova_recipe::{DetectRule, Recipe};
use serde::{Deserialize, Serialize};

use crate::{BundleError, FORMAT, Result};

/// Linux detect rules of every recipe, keyed by recipe id, so a client
/// can tell which apps are present without parsing every recipe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectIndex {
    /// Bundle format ([`FORMAT`]).
    pub format: u32,
    /// Recipe id → Linux detect rules (any match = detected).
    pub linux: BTreeMap<String, Vec<DetectRule>>,
}

impl DetectIndex {
    /// Collect the rules of `recipes`.
    pub fn from_recipes<'a>(recipes: impl IntoIterator<Item = &'a Recipe>) -> Self {
        Self {
            format: FORMAT,
            linux: recipes
                .into_iter()
                .map(|r| (r.id.clone(), r.detect.linux.clone()))
                .collect(),
        }
    }

    /// Deterministic JSON (sorted keys), so equal content has an equal hash.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec_pretty(self).map_err(BundleError::json("detect index"))
    }

    /// Parse detect-index bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(BundleError::json("detect index"))
    }
}
