//! Linked devices (ADR 0002): per-device feature policies and the QR
//! pairing invite.

mod invite;
mod policy;

pub use invite::PairInvite;
pub(crate) use policy::POLICY_DIR;
pub use policy::PolicyStore;
