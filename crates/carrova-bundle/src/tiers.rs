//! Trust tiers from `recipes/TIERS.yaml`.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::Deserialize;

use crate::{BundleError, Result, Tier};

/// Name of the tier policy file at the root of a recipe tree.
pub const TIERS_FILE: &str = "TIERS.yaml";

/// Which recipes are [`Tier::Verified`]; every other one is
/// [`Tier::Community`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TierPolicy {
    /// Ids of verified recipes.
    verified: BTreeSet<String>,
}

impl TierPolicy {
    /// Parse `TIERS.yaml` text.
    pub fn parse(text: &str) -> Result<Self> {
        serde_yaml_ng::from_str(text).map_err(|e| BundleError::Tiers(e.to_string()))
    }

    /// Read `<recipes_dir>/TIERS.yaml`; a missing file means everything is
    /// community.
    pub fn load(recipes_dir: &Path) -> Result<Self> {
        let path = recipes_dir.join(TIERS_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(BundleError::io(path, e)),
        }
    }

    /// Tier of recipe `id`.
    #[must_use]
    pub fn tier_of(&self, id: &str) -> Tier {
        if self.verified.contains(id) {
            Tier::Verified
        } else {
            Tier::Community
        }
    }

    /// Fail if the policy names a recipe that is not in `ids` (a typo
    /// would otherwise silently demote a recipe).
    pub fn check_known<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> Result<()> {
        let known: BTreeSet<&str> = ids.into_iter().collect();
        let unknown: Vec<&str> = self
            .verified
            .iter()
            .map(String::as_str)
            .filter(|id| !known.contains(id))
            .collect();
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(BundleError::Tiers(format!(
                "{TIERS_FILE} lists unknown recipe ids: {}",
                unknown.join(", ")
            )))
        }
    }
}
