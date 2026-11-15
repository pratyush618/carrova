//! Carrova link: per-device feature policy and the two-sided gate.

mod clipboard;
mod error;
mod file;
mod handoff;
mod name;
mod policy;
mod ring;

pub use clipboard::{ClipHistory, ClipItem, MAX_CLIP_PNG, MAX_CLIP_TEXT};
pub use error::LinkError;
pub use file::{FileOffer, OfferAnswer};
pub use handoff::{Handoff, SafeUrl, check_url};
pub use name::safe_file_name;
pub use policy::{Feature, FeatureState, Group, PolicyDoc, allowed};
pub use ring::{RING_SECONDS, RingMsg};
