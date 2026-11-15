//! Carrova link: per-device feature policy and the two-sided gate.

mod error;
mod policy;

pub use error::LinkError;
pub use policy::{Feature, FeatureState, Group, PolicyDoc, allowed};
