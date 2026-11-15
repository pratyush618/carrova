//! Lanes: typed bidirectional streams opened with a `LaneOpen` / `LaneReply` handshake.

use std::time::{Duration, Instant};

use carrova_transport::{Connection, Stream};
use serde::de::IgnoredAny;
use serde::de::value::{Error as ValueError, StrDeserializer};
use serde::{Deserialize, Serialize};

use crate::LinkError;

/// Lane protocol version this crate speaks.
const LANE_VERSION: u16 = 1;
/// How long an opener waits for the peer's verdict.
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);
/// Longest the acceptor waits for a lane's first frame (also capped by what is
/// left of the caller's accept timeout).
const OPEN_TIMEOUT: Duration = Duration::from_secs(3);

/// What a lane carries.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LaneKind {
    /// Policy documents.
    Policy,
    /// Clipboard items.
    Clipboard,
    /// File offers and bytes.
    File,
    /// URL handoff.
    Handoff,
    /// Ring requests.
    Ring,
    /// Round-trip probe.
    Ping,
}

/// First frame on a lane.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct LaneOpen {
    /// What the lane carries.
    pub kind: LaneKind,
    /// Lane protocol version (1).
    pub version: u16,
}

/// Why a lane was refused.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefuseReason {
    /// Unknown lane kind or version.
    Unknown,
    /// The feature is off for this link.
    NotAllowed,
    /// Too many lanes of this kind right now.
    Busy,
}

/// The acceptor's answer to a `LaneOpen`.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneReply {
    /// The lane is open.
    Accepted,
    /// The lane was refused.
    Refused(RefuseReason),
}

/// Local policy, consulted before anything past `LaneOpen` is read.
pub trait LaneGate {
    /// Whether a lane of `kind` may open now.
    fn admit(&self, kind: LaneKind) -> Result<(), RefuseReason>;
}

/// A field of a peer that may be newer: `None` when it is not the type this
/// version knows, so the lane can still be refused with a reply.
#[derive(Deserialize)]
#[serde(untagged)]
enum Lenient<T> {
    Known(T),
    Other(IgnoredAny),
}

impl<T> Lenient<T> {
    fn known(self) -> Option<T> {
        match self {
            Self::Known(v) => Some(v),
            Self::Other(_) => None,
        }
    }
}

/// A `LaneOpen` as decoded from a peer that may be newer.
#[derive(Deserialize)]
struct WireOpen {
    kind: Lenient<String>,
    version: Lenient<u64>,
}

impl WireOpen {
    fn kind(self) -> Option<LaneKind> {
        if self.version.known() != Some(u64::from(LANE_VERSION)) {
            return None;
        }
        // Reuses `LaneKind`'s own decoding, so the names cannot drift.
        let kind = self.kind.known()?;
        LaneKind::deserialize(StrDeserializer::<ValueError>::new(&kind)).ok()
    }
}

/// Open a lane of `kind` and wait for the peer to accept it.
pub fn open_lane(conn: &Connection, kind: LaneKind) -> Result<Stream, LinkError> {
    let mut stream = conn.open_stream()?;
    stream.send(&LaneOpen {
        kind,
        version: LANE_VERSION,
    })?;
    match stream.recv(REPLY_TIMEOUT)? {
        LaneReply::Accepted => Ok(stream),
        LaneReply::Refused(reason) => Err(LinkError::Refused(reason)),
    }
}

/// Accept the next lane, waiting up to `timeout` for it to appear. An unknown
/// kind or version, or a `gate` refusal, is answered with `Refused` and
/// returned as [`LinkError::Refused`]; the connection stays up.
pub fn accept_lane(
    conn: &Connection,
    gate: &dyn LaneGate,
    timeout: Duration,
) -> Result<(LaneKind, Stream), LinkError> {
    let start = Instant::now();
    let mut stream = conn.accept_stream(timeout)?;
    let first_frame = OPEN_TIMEOUT.min(timeout.saturating_sub(start.elapsed()));
    let open: WireOpen = stream.recv(first_frame)?;
    let verdict = open
        .kind()
        .ok_or(RefuseReason::Unknown)
        .and_then(|kind| gate.admit(kind).map(|()| kind));
    match verdict {
        Ok(kind) => {
            stream.send(&LaneReply::Accepted)?;
            Ok((kind, stream))
        }
        Err(reason) => {
            stream.send(&LaneReply::Refused(reason))?;
            stream.finish()?;
            Err(LinkError::Refused(reason))
        }
    }
}
