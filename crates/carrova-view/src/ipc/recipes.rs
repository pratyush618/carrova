//! Recipe catalogue and bundle payloads.

use carrova_bundle::{CheckAttempt, UpdateCheck};
use serde::Serialize;

/// Where the loaded recipes came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecipeOrigin {
    /// Unsigned developer tree (UI shows a warning).
    DevDir,
    /// Installed verified bundle.
    Installed {
        /// Bundle version.
        version: u64,
    },
    /// Bundle shipped inside the app.
    Shipped {
        /// Bundle version.
        version: u64,
    },
}

/// `recipes_status` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RecipesStatus {
    /// Source in use.
    pub origin: RecipeOrigin,
    /// Build time of the bundle (unix seconds); `None` for a dev tree.
    pub created: Option<u64>,
    /// Recipes in use whose effective tier is [`RecipeTier::Verified`].
    pub verified: u32,
    /// Recipes in use whose effective tier is [`RecipeTier::Community`].
    pub community: u32,
    /// Recipes in use whose effective tier is [`RecipeTier::Unverified`]
    /// (a developer tree: nothing in it is signed).
    pub unverified: u32,
    /// Ids skipped because this Carrova cannot read their schema.
    pub skipped: Vec<String>,
}

/// `recipes_update` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecipesUpdate {
    /// A newer bundle was verified and installed.
    Updated {
        /// New version.
        version: u64,
    },
    /// Already on the newest bundle.
    UpToDate,
    /// The release is being re-published; try again shortly.
    RetryLater,
}

/// `recipes_check` result: whether a newer bundle is offered (nothing is
/// installed). The CLI's `recipes update --check` reads the same value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecipesCheck {
    /// A newer, verified bundle is ready to install (`recipes_update`).
    Available {
        /// Offered version.
        version: u64,
    },
    /// Already on the newest bundle.
    UpToDate {
        /// Installed (and offered) version.
        version: u64,
    },
    /// The offered bundle was rolled back by the user, so it is not offered
    /// again.
    Refused {
        /// Offered version.
        version: u64,
    },
    /// The release is being re-published; try again shortly.
    RetryLater,
}

impl From<CheckAttempt> for RecipesCheck {
    fn from(attempt: CheckAttempt) -> Self {
        match attempt {
            CheckAttempt::RetryLater => Self::RetryLater,
            CheckAttempt::Done(UpdateCheck::Available(pending)) => Self::Available {
                version: pending.version().0,
            },
            CheckAttempt::Done(UpdateCheck::UpToDate(v)) => Self::UpToDate { version: v.0 },
            CheckAttempt::Done(UpdateCheck::Refused(v)) => Self::Refused { version: v.0 },
        }
    }
}

/// A recipe's effective trust tier (carrova-bundle's `EffectiveTier`), as
/// the catalogue and [`RecipesStatus`] count it: a recipe from an
/// unsigned developer tree is always [`Self::Unverified`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RecipeTier {
    /// Reviewed by maintainers.
    Verified,
    /// Contributed, not yet verified.
    Community,
    /// From an unsigned developer tree.
    Unverified,
}

/// One app in the "Apps Carrova knows" catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CatalogEntry {
    /// Recipe id (`git`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Category dir (`dev-tools`).
    pub category: String,
    /// One-line description.
    pub description: String,
    /// Effective trust tier.
    pub tier: RecipeTier,
    /// Installed on this machine.
    pub detected: bool,
    /// What the recipe moves.
    pub items: Vec<CatalogItem>,
}

/// One item of a [`CatalogEntry`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CatalogItem {
    /// Item id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Suggested on by default.
    pub default: bool,
    /// Holds secrets: off until confirmed.
    pub sensitive: bool,
    /// Never moves (listed with its reason).
    pub unmovable: bool,
}
