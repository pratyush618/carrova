//! Linked devices (ADR 0002): per-device feature policies, the QR pairing
//! invite, pair-only linking and the link supervisor (one per user,
//! shared over a control socket).

mod asks;
mod clip;
mod control;
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
pub use control::{
    ControlClient, ControlError, ControlRequest, ControlResponse, ControlServer,
    MAX_CONTROL_MESSAGE, SupervisorHandle, attach_or_start, attach_or_start_with,
};
pub use features::FileFailure;
pub use invite::PairInvite;
pub use pair::{accept_invite, link_with_code, offer_pairing};
pub use peer::{LinkedPeer, PeerSource, PeerState, PresenceSource, SeenPeer};
pub(crate) use policy::POLICY_DIR;
pub use policy::PolicyStore;
pub use supervisor::{HandoffItem, Supervisor, SupervisorConfig};
