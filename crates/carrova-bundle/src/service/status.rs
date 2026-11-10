//! Which recipes are in use: the status both front-ends show.

use crate::{RecipeSource, ResolvedRecipes, SkippedRecipe, TierCounts};

/// Source, trust, age, counts and skipped recipes of a
/// [`ResolvedRecipes`] (CLI `carrova recipes status`, desktop settings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipesInUse {
    /// Where the recipes come from (dev tree path or bundle version).
    pub source: RecipeSource,
    /// Whether the source is signature-verified.
    pub verified: bool,
    /// When the bundle was built (unix seconds); `None` for a dev tree.
    pub created: Option<u64>,
    /// Usable recipes by effective tier.
    pub counts: TierCounts,
    /// Recipes left out, with the reason.
    pub skipped: Vec<SkippedRecipe>,
}

impl ResolvedRecipes {
    /// What these recipes look like to the user.
    #[must_use]
    pub fn in_use(&self) -> RecipesInUse {
        RecipesInUse {
            source: self.source.clone(),
            verified: self.source.is_verified(),
            created: self.created,
            counts: self.tier_counts(),
            skipped: self.skipped.clone(),
        }
    }
}
