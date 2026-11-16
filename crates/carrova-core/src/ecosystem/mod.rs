//! Linked devices (ADR 0002): per-device feature policies, the QR pairing
//! invite, pair-only linking and the link supervisor (one per user,
//! shared over a control socket).

mod asks;
mod clip;
mod clip_settings;
mod control;
mod dial;
mod features;
mod invite;
mod pair;
mod peer;
mod phone;
mod policy;
mod retry;
mod sink;
mod supervisor;

pub use asks::{Answer, AskId, AskKind};
pub use clip::ClipSkip;
pub use clip_settings::ClipSettings;
pub use control::{
    ControlClient, ControlError, ControlRequest, ControlResponse, ControlServer,
    MAX_CONTROL_MESSAGE, SharedSupervisor, SupervisorHandle, attach_or_start, attach_or_start_with,
};
pub use features::FileFailure;
pub use invite::PairInvite;
pub use pair::{PAIR_CONFIRM_WINDOW, PeerFound, accept_invite, link_with_code, offer_pairing};
pub use peer::{LinkedPeer, PeerSource, PeerState, PresenceSource, SeenPeer};
pub use phone::{CallView, NotifyApps, PhoneSettings, RingHere};
pub(crate) use policy::POLICY_DIR;
pub use policy::PolicyStore;
pub use supervisor::{HandoffItem, Supervisor, SupervisorConfig};
