//! One request, one reply on a fresh lane: the shape the phone lanes share.
//!
//! Wire (opener = asker), frames are length + CBOR: asker → `Q`; server acts,
//! then → `R` and finishes.

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

/// Read the request on an accepted lane; answer it later through the
/// returned [`Responder`].
///
/// # Errors
///
/// [`LinkError`] if no well-formed request arrives in time.
pub fn serve<Q: DeserializeOwned, R: Serialize>(
    mut stream: Stream,
) -> Result<(Q, Responder<R>), LinkError> {
    let request = stream.recv(FRAME_TIMEOUT)?;
    Ok((
        request,
        Responder {
            stream,
            reply: PhantomData,
        },
    ))
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
