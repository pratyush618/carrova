//! The tier a user is shown for a recipe.

use serde::Serialize;

use crate::{BundledRecipe, ResolvedRecipes, Tier};

/// A recipe's trust as shown to the user: its bundle [`Tier`] when the
/// source is signature-verified, else [`Self::Unverified`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveTier {
    /// Reviewed by maintainers, from a verified bundle.
    Verified,
    /// Contributed, not yet reviewed, from a verified bundle.
    Community,
    /// From an unsigned recipe tree (developer override or checkout).
    Unverified,
}

impl From<Tier> for EffectiveTier {
    fn from(tier: Tier) -> Self {
        match tier {
            Tier::Verified => Self::Verified,
            Tier::Community => Self::Community,
        }
    }
}

/// Usable recipes by [`EffectiveTier`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct TierCounts {
    /// All usable recipes.
    pub total: usize,
    /// [`EffectiveTier::Verified`] recipes.
    pub verified: usize,
    /// [`EffectiveTier::Community`] recipes.
    pub community: usize,
    /// [`EffectiveTier::Unverified`] recipes.
    pub unverified: usize,
}

impl TierCounts {
    fn add(mut self, tier: EffectiveTier) -> Self {
        self.total += 1;
        match tier {
            EffectiveTier::Verified => self.verified += 1,
            EffectiveTier::Community => self.community += 1,
            EffectiveTier::Unverified => self.unverified += 1,
        }
        self
    }
}

impl ResolvedRecipes {
    /// The tier shown for `recipe` (one of [`Self::recipes`]): any recipe
    /// from an unsigned source is [`EffectiveTier::Unverified`], whatever
    /// its `TIERS.yaml` entry says.
    #[must_use]
    pub fn effective_tier(&self, recipe: &BundledRecipe) -> EffectiveTier {
        if self.source.is_verified() {
            recipe.tier.into()
        } else {
            EffectiveTier::Unverified
        }
    }

    /// [`Self::recipes`] counted by [`Self::effective_tier`].
    #[must_use]
    pub fn tier_counts(&self) -> TierCounts {
        self.recipes
            .iter()
            .map(|r| self.effective_tier(r))
            .fold(TierCounts::default(), TierCounts::add)
    }
}
