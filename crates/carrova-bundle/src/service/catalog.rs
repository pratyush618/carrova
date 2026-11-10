//! The "apps Carrova knows" catalogue.

use carrova_platform::Platform;
use carrova_recipe::Recipe;

use crate::{EffectiveTier, ResolvedRecipes};

/// One app of the catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownApp<'a> {
    /// Its recipe.
    pub recipe: &'a Recipe,
    /// Trust shown to the user ([`ResolvedRecipes::effective_tier`]).
    pub tier: EffectiveTier,
    /// Installed on this machine (the recipe's detect rules match).
    pub detected: bool,
}

impl ResolvedRecipes {
    /// Every usable recipe as a catalogue entry, in id order, with
    /// detection on `platform`.
    #[must_use]
    pub fn catalog(&self, platform: &dyn Platform) -> Vec<KnownApp<'_>> {
        self.recipes
            .iter()
            .map(|bundled| KnownApp {
                recipe: &bundled.recipe,
                tier: self.effective_tier(bundled),
                detected: carrova_recipe::detect(&bundled.recipe, platform).detected,
            })
            .collect()
    }
}
