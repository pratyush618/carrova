//! Link errors.

use std::io;

use carrova_transport::TransportError;

use crate::lane::RefuseReason;
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
    /// The peer refused the lane, or this side refused the peer's.
    #[error("lane refused: {0:?}")]
    Refused(RefuseReason),
    /// The receiver declined the file offer.
    #[error("the other device declined the file")]
    Declined,
    /// The file offer went unanswered.
    #[error("the file offer expired")]
    Expired,
    /// The peer broke a lane protocol.
    #[error("protocol error: {0}")]
    Protocol(&'static str),
    /// Reading the source or writing the sink failed.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// The transport failed.
    #[error(transparent)]
    Transport(#[from] TransportError),
}
