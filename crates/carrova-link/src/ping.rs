//! Round-trip probe on a `Ping` lane.

use std::time::{Duration, Instant};

use carrova_transport::{Connection, Stream};

use crate::LinkError;
use crate::lane::{LaneKind, open_lane};

/// How long either side waits for the other's frame.
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Round-trip time to the peer, measured on a fresh `Ping` lane.
pub fn ping(conn: &Connection) -> Result<Duration, LinkError> {
    let mut stream = open_lane(conn, LaneKind::Ping)?;
    let start = Instant::now();
    stream.send(&())?;
    stream.recv::<()>(PING_TIMEOUT)?;
    Ok(start.elapsed())
}

/// Answer one ping on an accepted `Ping` lane.
pub fn serve_ping(mut stream: Stream) -> Result<(), LinkError> {
    stream.recv::<()>(PING_TIMEOUT)?;
    stream.send(&())?;
    stream.finish()?;
    Ok(())
}
