//! One request, one reply on a fresh lane: the shape the phone lanes share.
//!
//! Wire (opener = asker), frames are length + CBOR: asker → `Q`; server acts,
//! then → `R` (a reply) or `()` (an ack of a one-way frame) and finishes.

use std::marker::PhantomData;
use std::time::Duration;

use carrova_transport::{Connection, Stream};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::LinkError;
use crate::lane::{FRAME_TIMEOUT, LaneKind, open_lane};

/// Send `request` on a new `kind` lane and wait up to `wait` for the reply.
///
/// # Errors
///
/// [`LinkError`] if the lane is refused, the peer is gone or the reply is
/// late or malformed.
pub fn request<Q: Serialize, R: DeserializeOwned>(
    conn: &Connection,
    kind: LaneKind,
    request: &Q,
    wait: Duration,
) -> Result<R, LinkError> {
    let mut stream = open_lane(conn, kind)?;
    stream.send(request)?;
    Ok(stream.recv(wait)?)
}

/// Read the first frame on an accepted lane. The returned [`Pending`] then
/// either acknowledges it ([`Pending::ack`], for one-way pushes) or turns
/// into a typed [`Responder`] (for requests), so one lane kind can carry
/// both.
///
/// # Errors
///
/// [`LinkError`] if no well-formed frame arrives in time.
pub fn serve<Q: DeserializeOwned>(mut stream: Stream) -> Result<(Q, Pending), LinkError> {
    let frame = stream.recv(FRAME_TIMEOUT)?;
    Ok((frame, Pending { stream }))
}

/// An accepted lane whose first frame was read and not yet answered.
#[derive(Debug)]
pub struct Pending {
    stream: Stream,
}

impl Pending {
    /// Acknowledge a one-way frame with `()` and close the lane; the sender
    /// reads it with [`request`]`::<_, ()>`.
    ///
    /// # Errors
    ///
    /// [`LinkError`] if the peer is gone.
    pub fn ack(mut self) -> Result<(), LinkError> {
        self.stream.send(&())?;
        self.stream.finish()?;
        Ok(())
    }

    /// The responder for a request whose reply type is `R`.
    #[must_use]
    pub fn responder<R: Serialize>(self) -> Responder<R> {
        Responder {
            stream: self.stream,
            reply: PhantomData,
        }
    }
}

/// The open lane a request came in on, waiting for its one reply.
#[derive(Debug)]
pub struct Responder<R> {
    stream: Stream,
    reply: PhantomData<fn(R)>,
}

impl<R: Serialize> Responder<R> {
    /// Send the reply and close the lane.
    ///
    /// # Errors
    ///
    /// [`LinkError`] if the peer is gone.
    pub fn reply(mut self, reply: &R) -> Result<(), LinkError> {
        self.stream.send(reply)?;
        self.stream.finish()?;
        Ok(())
    }
}
