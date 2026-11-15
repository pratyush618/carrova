//! Lanes: bidirectional streams either side opens on a [`Connection`].
//!
//! [`Connection`]: crate::Connection

use std::time::Duration;

use quinn::SendStream;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::time::{Instant, timeout, timeout_at};

use crate::cancel::Watch;
use crate::error::{Result, TransportError};
use crate::tuning::{FRAME_WRITE_TIMEOUT, STREAM_OPEN_TIMEOUT};
use crate::wire::{STREAM_CANCELLED, StreamOpen, StreamReader, encode_frame};

/// One lane: control frames (length + CBOR, at most
/// [`crate::MAX_FRAME_LEN`]) and raw bytes, in order, both ways. Blocking;
/// every call watches the connection's cancel signal. Dropping it finishes
/// it (quinn 0.11), so the peer sees a dropped lane as
/// [`TransportError::StreamFinished`] too: protocols must end with an
/// explicit end/ack frame to tell completion from abandonment.
#[derive(Debug)]
pub struct Stream {
    send: SendStream,
    recv: StreamReader,
    cancel: Watch,
}

impl Stream {
    /// Send one frame, waiting at most 30 s for the
    /// peer to make room (see [`Self::write_all`]).
    pub fn send<T: Serialize>(&mut self, msg: &T) -> Result<()> {
        let frame = encode_frame(msg)?;
        self.write_all(&frame, FRAME_WRITE_TIMEOUT)
    }

    /// Receive the next frame, waiting at most `timeout_after`. A timeout
    /// or cancel loses nothing (a partial frame stays buffered). The peer
    /// finishing the lane is [`TransportError::StreamFinished`].
    pub fn recv<T: DeserializeOwned>(&mut self, timeout_after: Duration) -> Result<T> {
        let recv = &mut self.recv;
        self.cancel.block_on(async {
            timeout(timeout_after, recv.frame())
                .await
                .map_err(|_| TransportError::Timeout)??
                .ok_or(TransportError::StreamFinished)
        })
    }

    /// Write raw bytes, waiting at most `timeout_after` for a peer that
    /// stopped reading ([`TransportError::Timeout`]). After a timeout part
    /// of `bytes` may be sent, so the lane is unusable: drop it.
    pub fn write_all(&mut self, bytes: &[u8], timeout_after: Duration) -> Result<()> {
        let send = &mut self.send;
        self.cancel.block_on(async {
            timeout(timeout_after, send.write_all(bytes))
                .await
                .map_err(|_| TransportError::Timeout)?
                .map_err(|e| TransportError::write(&e))
        })
    }

    /// Fill `buf` with raw bytes, waiting at most `timeout_after`. A
    /// timeout or cancel loses nothing (bytes read so far stay buffered,
    /// so keep `buf` modest). The peer finishing the lane first is
    /// [`TransportError::StreamFinished`] (before any byte) or a truncated
    /// message (part-way).
    pub fn read_exact(&mut self, buf: &mut [u8], timeout_after: Duration) -> Result<()> {
        let recv = &mut self.recv;
        self.cancel.block_on(async {
            let filled = timeout(timeout_after, recv.exact(buf))
                .await
                .map_err(|_| TransportError::Timeout)??;
            filled.then_some(()).ok_or(TransportError::StreamFinished)
        })
    }

    /// Finish this side of the lane: the peer reads what was sent, then
    /// [`TransportError::StreamFinished`].
    pub fn finish(mut self) -> Result<()> {
        self.send
            .finish()
            // Only fails if finished or reset before, which `Stream` never does.
            .map_err(|e| TransportError::internal("finish lane", e))
    }
}

/// Open a lane on `conn`: tagged at once, so the peer's accept sees it.
pub(crate) async fn open(conn: &quinn::Connection, cancel: &Watch) -> Result<Stream> {
    let (mut send, recv) = conn
        .open_bi()
        .await
        .map_err(|e| TransportError::connection(&e))?;
    send.write_all(&encode_frame(&StreamOpen::Lane)?)
        .await
        .map_err(|e| TransportError::write(&e))?;
    Ok(Stream {
        send,
        recv: StreamReader::new(recv),
        cancel: cancel.clone(),
    })
}

/// Accept the next lane on `conn` before `deadline`. Other streams (a move
/// session's chunk streams, unknown tags) are reset and skipped.
pub(crate) async fn accept(
    conn: &quinn::Connection,
    cancel: &Watch,
    deadline: Instant,
) -> Result<Stream> {
    loop {
        // Streams already queued would otherwise win over a passed deadline.
        if Instant::now() >= deadline {
            return Err(TransportError::Timeout);
        }
        let (mut send, recv) = timeout_at(deadline, conn.accept_bi())
            .await
            .map_err(|_| TransportError::Timeout)?
            .map_err(|e| TransportError::connection(&e))?;
        let mut recv = StreamReader::new(recv);
        // Its own bound: a lane accepted just before the deadline is kept.
        let tag = timeout(STREAM_OPEN_TIMEOUT, recv.expect_frame::<StreamOpen>()).await;
        if let Ok(Ok(StreamOpen::Lane)) = tag {
            return Ok(Stream {
                send,
                recv,
                cancel: cancel.clone(),
            });
        }
        tracing::debug!("non-lane stream refused");
        let _ = send.reset(STREAM_CANCELLED);
        let _ = recv.stream_mut().stop(STREAM_CANCELLED);
    }
}

#[cfg(test)]
#[path = "../tests/stream/unit.rs"]
mod tests;
