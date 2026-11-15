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
    /// A payload exceeds its size limit.
    #[error("{len} bytes exceeds the {max} byte limit")]
    TooBig {
        /// Offending size in bytes.
        len: usize,
        /// Limit it broke.
        max: usize,
    },
    /// A handed-off URL is not an absolute http(s) URL with a host.
    #[error("only http and https URLs with a host can be handed off")]
    UnsafeUrl,
}
