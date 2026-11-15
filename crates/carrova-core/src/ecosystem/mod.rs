//! Linked devices (ADR 0002): per-device feature policies and the QR
//! pairing invite.

mod invite;
mod pair;
mod policy;

pub use invite::PairInvite;
pub use pair::{accept_invite, link_with_code, offer_pairing};
pub(crate) use policy::POLICY_DIR;
pub use policy::PolicyStore;
