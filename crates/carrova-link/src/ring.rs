//! Ring-a-device messages and the `Ring` lane.
//!
//! Wire (opener = ringer), frames are length + CBOR: sender → `RingMsg`;
//! receiver → `()` and finishes. Who may ring whom is the supervisor's policy.

use std::time::Duration;

use carrova_transport::{Connection, Stream};
use serde::{Deserialize, Serialize};

use crate::LinkError;
use crate::lane::{LaneKind, open_lane};

/// How long either side waits for the other's frame.
const RING_TIMEOUT: Duration = Duration::from_secs(5);

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

/// Send `msg` on a new `Ring` lane; succeeds once the peer acknowledges it.
pub fn send_ring(conn: &Connection, msg: RingMsg) -> Result<(), LinkError> {
    let mut stream = open_lane(conn, LaneKind::Ring)?;
    stream.send(&msg)?;
    stream.recv::<()>(RING_TIMEOUT)?;
    Ok(())
}

/// Receive a ring request on an accepted `Ring` lane and acknowledge it. A
/// `Start` longer than [`RING_SECONDS`] is clamped: a peer can't ring for hours.
pub fn recv_ring(mut stream: Stream) -> Result<RingMsg, LinkError> {
    let msg = match stream.recv(RING_TIMEOUT)? {
        RingMsg::Start { seconds } => RingMsg::Start {
            seconds: seconds.min(RING_SECONDS),
        },
        RingMsg::Stop => RingMsg::Stop,
    };
    stream.send(&())?;
    stream.finish()?;
    Ok(msg)
}
