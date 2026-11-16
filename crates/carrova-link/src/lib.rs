//! Carrova link: per-device feature policy and the two-sided gate.

mod clipboard;
mod error;
mod file;
mod handoff;
mod lane;
mod name;
mod ping;
mod policy;
mod reply;
mod ring;

pub use carrova_platform::phone::{AudioSide, SendOutcome};
pub use clipboard::{
    ClipHistory, ClipItem, MAX_CLIP_PNG, MAX_CLIP_TEXT, ReceivedClip, recv_clip, send_clip,
};
pub use error::LinkError;
pub use file::{
    ANSWER_TIMEOUT, FileOffer, FileSink, OfferAnswer, ReadSeek, offer_file, receive_file,
};
pub use handoff::{Handoff, ReceivedHandoff, SafeUrl, check_url, recv_handoff, send_handoff};
pub use lane::{LaneGate, LaneKind, LaneOpen, LaneReply, RefuseReason, accept_lane, open_lane};
pub use name::safe_file_name;
pub use ping::{ping, serve_ping};
pub use policy::{
    Feature, FeatureState, Group, PolicyDoc, PolicyUpdate, allowed, recv_policy, send_policy,
};
pub use reply::{Responder, request, serve};
pub use ring::{RING_SECONDS, RingMsg, recv_ring, send_ring};
