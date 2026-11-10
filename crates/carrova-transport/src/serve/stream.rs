//! One chunk stream: check requests, queue reads, write records in order.

use std::sync::Arc;

use carrova_manifest::ChunkRef;
use quinn::SendStream;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};

use super::outbox::{Outbox, Respond};
use super::quota::Quota;
use super::{ReadJob, StreamEnd};
use crate::connection::{AllowedChunks, ServeEnd};
use crate::error::{ProtocolViolation, Result, TransportError};
use crate::runtime::Stop;
use crate::tuning::{RESPONSE_COALESCE_BYTES, SERVE_STREAM_BUDGET_BYTES, budget_permits};
use crate::wire::{
    ChunkRequest, RECORD_HEADER_LEN, STREAM_CANCELLED, StreamOpen, StreamReader, chunk_len,
};

/// A queued read whose record is written in request order.
struct Pending {
    chunk: ChunkRef,
    result: oneshot::Receiver<Option<Vec<u8>>>,
    _budget: OwnedSemaphorePermit,
}

pub(super) async fn stream(
    mut send: SendStream,
    recv: quinn::RecvStream,
    allowed: AllowedChunks,
    quota: Quota,
    jobs: mpsc::Sender<ReadJob>,
    stop: Arc<Stop>,
) -> Result<StreamEnd> {
    let mut recv = StreamReader::new(recv);
    match recv.expect_frame::<StreamOpen>().await? {
        StreamOpen::FetchDone => return Ok(StreamEnd::Done(send)),
        StreamOpen::Chunks => {}
    }
    let (pending_tx, pending_rx) = mpsc::unbounded_channel();
    let mut sent = ServeEnd::default();
    // Stop first: a stop also closes the read queue, which would otherwise
    // let `respond` answer the rest "unavailable" and finish the stream
    // cleanly, so the receiver would see a truncated stream instead of a
    // reset followed by the cancel close.
    let outcome = tokio::select! {
        biased;
        () = stop.stopped() => None,
        outcome = async {
            tokio::join!(
                requests(&mut recv, &allowed, &quota, &jobs, pending_tx),
                respond(&mut send, pending_rx, &mut sent),
            )
        } => Some(outcome),
    };
    // The join can also run dry because serving stopped (the read queue
    // closes right after `stop`) before `stopped()` is polled again: reset
    // then too, never finish, or the receiver sees a truncated stream.
    let outcome = outcome.filter(|_| !stop.is_stopped());
    let Some((requested, responded)) = outcome else {
        let _ = send.reset(STREAM_CANCELLED);
        let _ = recv.stream_mut().stop(STREAM_CANCELLED);
        return Ok(StreamEnd::Served(sent));
    };
    match responded {
        // The receiver stopped reading (cancelled fetch): whatever the
        // request side saw is moot.
        Err(Respond::Stopped) => Ok(StreamEnd::Served(sent)),
        Err(Respond::Failed(e)) => Err(e),
        Ok(()) => {
            let _ = send.finish();
            requested.map(|()| StreamEnd::Served(sent))
        }
    }
}

/// Read request frames, check each hash against the manifest (and its
/// length against the chunk limit) and the connection's quota, queue reads.
async fn requests(
    recv: &mut StreamReader,
    allowed: &AllowedChunks,
    quota: &Quota,
    jobs: &mpsc::Sender<ReadJob>,
    pending: mpsc::UnboundedSender<Pending>,
) -> Result<()> {
    let budget = Arc::new(Semaphore::new(SERVE_STREAM_BUDGET_BYTES));
    loop {
        let request: ChunkRequest = match recv.frame().await {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(()),
            Err(_) if recv.reset_by_peer() => return Ok(()),
            Err(e) => return Err(e),
        };
        for hash in request.hashes {
            let chunk = *allowed
                .get(&hash)
                .ok_or_else(|| TransportError::from(ProtocolViolation::NotInManifest(hash)))?;
            let len = chunk_len(chunk.len)?;
            quota.take(chunk.len)?;
            let permit = Arc::clone(&budget)
                .acquire_many_owned(budget_permits(len, SERVE_STREAM_BUDGET_BYTES))
                .await
                .map_err(|e| TransportError::internal("serve budget", e))?;
            let (reply, result) = oneshot::channel();
            let queued = jobs.send(ReadJob { chunk, reply }).await.is_ok()
                && pending
                    .send(Pending {
                        chunk,
                        result,
                        _budget: permit,
                    })
                    .is_ok();
            if !queued {
                // Serving or this stream's writer ended.
                return Ok(());
            }
        }
    }
}

/// Write records in request order, coalescing small ones; `sent` counts
/// what reached the stream.
async fn respond(
    send: &mut SendStream,
    mut pending: mpsc::UnboundedReceiver<Pending>,
    sent: &mut ServeEnd,
) -> Result<(), Respond> {
    let mut out = Outbox::new();
    loop {
        let next = match pending.try_recv() {
            Ok(p) => Some(p),
            Err(TryRecvError::Empty) => {
                out.flush(send, sent).await?;
                pending.recv().await
            }
            Err(TryRecvError::Disconnected) => None,
        };
        let Some(p) = next else { break };
        // A dropped reply means serving is ending: report unavailable.
        let data = p.result.await.ok().flatten();
        let len = data.as_ref().map_or(0, Vec::len);
        if !out.buf.is_empty() && out.buf.len() + RECORD_HEADER_LEN + len > RESPONSE_COALESCE_BYTES
        {
            out.flush(send, sent).await?;
        }
        out.push(&p.chunk, data);
    }
    // The caller finishes the stream, once it knows serving wasn't stopped.
    out.flush(send, sent).await?;
    Ok(())
}
