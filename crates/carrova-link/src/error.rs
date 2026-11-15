//! Link errors.

use crate::policy::{Feature, FeatureState};

/// Failures of the link layer.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    /// A policy document holds a state its feature's group does not allow.
    #[error("state {state:?} is not valid for feature {feature:?}")]
    InvalidState {
        /// Offending feature.
        feature: Feature,
        /// State it was given.
        state: FeatureState,
    },
}
