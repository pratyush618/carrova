//! Linked devices (ADR 0002): per-device feature policies, the QR pairing
//! invite, pair-only linking and the link supervisor.

mod asks;
mod clip;
mod dial;
mod features;
mod invite;
mod pair;
mod peer;
mod policy;
mod sink;
mod supervisor;

pub use asks::{Answer, AskId, AskKind};
pub use clip::ClipSkip;
pub use features::FileFailure;
pub use invite::PairInvite;
pub use pair::{accept_invite, link_with_code, offer_pairing};
pub use peer::{LinkedPeer, PeerSource, PeerState, PresenceSource, SeenPeer};
pub(crate) use policy::POLICY_DIR;
pub use policy::PolicyStore;
pub use supervisor::{HandoffItem, Supervisor, SupervisorConfig};
