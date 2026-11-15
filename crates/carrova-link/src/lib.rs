//! Carrova link: per-device feature policy and the two-sided gate.

mod clipboard;
mod error;
mod policy;

pub use clipboard::{ClipHistory, ClipItem, MAX_CLIP_PNG, MAX_CLIP_TEXT};
pub use error::LinkError;
pub use policy::{Feature, FeatureState, Group, PolicyDoc, allowed};
