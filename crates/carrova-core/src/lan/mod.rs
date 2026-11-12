//! LAN session wire messages and transfer ordering.
//!
//! After pairing, the peers exchange [`LanMsg`]s on the connection's control
//! stream. The receiver drives; the sender only answers (its state machine
//! is a small state machine in `protocol.rs`):
//!
//! ```text
//! receiver → sender   Inventory        (first message after pairing)
//! sender   → receiver Manifest × n     (CBOR manifest bytes, in parts)
//! receiver → sender   PlanSummary      (on approval; again after a re-plan)
//! receiver → sender   Fetch            (one per batch; the sender serves
//!                                       chunks until the receiver's fetch ends)
//! receiver → sender   Applied          (the receiver's checklist)
//! receiver → sender   Abort            (session ends early, e.g. dry run)
//! ```
//!
//! After a reconnect (resume secret) the receiver's first message is
//! `Resume { stage }`, saying where it is; the sender keeps no protocol
//! state across connections, so a restarted sender continues the same way.
//!
//! The receiver plans locally; only the [`PlanSummary`] of the selected
//! items leaves it, never its file listing.

mod limits;
pub(crate) mod link;
mod messages;
mod nearby;
mod order;
mod protocol;
mod summary;

pub use limits::{
    BROWSE_WINDOW, CODE_LIFETIME, FINAL_NOTICE_WINDOW, IDLE_POLL, MANIFEST_PART_LEN,
    MAX_PAIRING_FAILURES, MIN_FREE_DURING_TRANSFER, PAIR_TIMEOUT, RECONNECT_ATTEMPT,
    RECONNECT_BACKOFF_MAX, RECONNECT_BACKOFF_MIN, RECONNECT_WINDOW, REPLY_TIMEOUT,
    SPACE_POLL_INTERVAL, TRANSFER_BATCH_BYTES, check_space,
};
pub(crate) use link::Link;
pub use messages::{AbortReason, Inventory, InventoryApp, LanMsg, MAX_DEVICE_NAME, ReceiverStage};
pub(crate) use messages::{ReceiverFailure, clean_device_name};
pub(crate) use nearby::known_by_recent_use;
pub use nearby::{NearbyDevice, browse_nearby};
pub use order::transfer_order;
pub use protocol::LanViolation;
pub(crate) use protocol::{
    SendAction, SendStep, farewell_cancelled, manifest_messages, recv_manifest,
};
pub use summary::{ActionCounts, ConflictTarget, ItemPlanSummary, PlanSummary};
