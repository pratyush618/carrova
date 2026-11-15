//! Ring-a-device messages.

use serde::{Deserialize, Serialize};

/// Default ring length in seconds.
pub const RING_SECONDS: u16 = 30;

/// A ring request or its cancellation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RingMsg {
    /// Start ringing for `seconds`.
    Start {
        /// Ring length.
        seconds: u16,
    },
    /// Stop ringing.
    Stop,
}
