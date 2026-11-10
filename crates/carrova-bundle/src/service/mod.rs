//! The recipes service every front-end wraps (CLI `carrova recipes …`, the
//! desktop catalogue and settings): one implementation of what the user
//! sees about the recipes in use, so both front-ends agree.
//!
//! * [`ResolvedRecipes::effective_tier`]: the tier shown for a recipe;
//!   anything from an unsigned source is [`EffectiveTier::Unverified`],
//!   whatever `TIERS.yaml` says.
//! * [`ResolvedRecipes::in_use`]: source, version, creation time, counts by
//!   effective tier and skipped recipes ([`RecipesInUse`]).
//! * [`ResolvedRecipes::catalog`]: every known app with its effective tier
//!   and whether it is detected here ([`KnownApp`]).
//! * [`update`] / [`update_with`]: open the store, follow the release
//!   pointer, verify and install; a bundle that is not (yet) published is
//!   [`UpdateAttempt::RetryLater`], not a failure.
//!
//! Front-ends map these bundle-native types to their own output (CLI text
//! and JSON, desktop IPC types); this crate never depends on a front-end
//! crate.

mod catalog;
mod status;
mod tier;
mod update;

pub use catalog::KnownApp;
pub use status::RecipesInUse;
pub use tier::{EffectiveTier, TierCounts};
pub use update::{UpdateAttempt, update, update_with};
