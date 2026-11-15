//! Carrova link: per-device feature policy and the two-sided gate.

mod clipboard;
mod error;
mod file;
mod name;
mod policy;

pub use clipboard::{ClipHistory, ClipItem, MAX_CLIP_PNG, MAX_CLIP_TEXT};
pub use error::LinkError;
pub use file::{FileOffer, OfferAnswer};
pub use name::safe_file_name;
pub use policy::{Feature, FeatureState, Group, PolicyDoc, allowed};
