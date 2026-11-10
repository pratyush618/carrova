//! Chunk streams on the runtime: request batches, read records, and the
//! closing fetch-done exchange.

use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use tokio::task::JoinSet;

use super::deliver::Delivery;
use super::work::{Job, Work};
use crate::error::{ProtocolViolation, Result, TransportError};
use crate::runtime::Stoppable;
use crate::tuning::{DELIVERY_BUDGET_BYTES, STREAM_OUTSTANDING_BYTES, budget_permits};
use crate::wire::{
    ChunkRequest, CloseCode, RecordKind, STREAM_CANCELLED, StreamOpen, StreamReader, chunk_len,
    encode_frame,
};

/// Run the chunk streams, then tell the sender the fetch phase is over.
pub(super) async fn drive(
    conn: quinn::Connection,
    work: Arc<Work>,
    tx: mpsc::UnboundedSender<Delivery>,
    parallel: usize,
) -> Result<()> {
    let budget = Arc::new(Semaphore::new(DELIVERY_BUDGET_BYTES));
    let mut streams = JoinSet::new();
    for _ in 0..parallel {
        streams.spawn(stream(
            conn.clone(),
            Arc::clone(&work),
            tx.clone(),
            Arc::clone(&budget),
        ));
    }
    drop(tx);
    let mut failure = None;
    while let Some(joined) = streams.join_next().await {
        let result = joined
            .map_err(|e| TransportError::internal("chunk stream task", e))
            .and_then(|r| r);
        if let Err(e) = result
            && failure.is_none()
        {
            work.stop();
            failure = Some(e);
        }
    }
    if let Some(e) = failure {
        if e.is_violation() {
            CloseCode::Protocol.close(&conn);
        }
        return Err(e);
    }
    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| TransportError::connection(&e))?;
    send.write_all(&encode_frame(&StreamOpen::FetchDone)?)
        .await
        .map_err(|e| TransportError::write(&e))?;
    let _ = send.finish();
    // The sender finishes the stream once it has drained every chunk
    // stream: after this, both sides agree the fetch phase is over.
    recv.read_to_end(0).await.map_err(|e| match e {
        quinn::ReadToEndError::Read(e) => TransportError::read(&e),
        quinn::ReadToEndError::TooLong => TransportError::from(ProtocolViolation::FetchDoneData),
    })?;
    Ok(())
}

/// One chunk stream: request batches, read records, pass them on.
async fn stream(
    conn: quinn::Connection,
    work: Arc<Work>,
    tx: mpsc::UnboundedSender<Delivery>,
    budget: Arc<Semaphore>,
) -> Result<()> {
    // Open the stream only once there is work for it.
    let Some(first) = work.next_batch().await else {
        return Ok(());
    };
    let (mut send, recv) = conn
        .open_bi()
        .await
        .map_err(|e| TransportError::connection(&e))?;
    let mut recv = StreamReader::new(recv);
    let (expect_tx, expect_rx) = mpsc::unbounded_channel();

    let outcome = tokio::select! {
        result = async {
            tokio::try_join!(
                request(&mut send, &work, first, expect_tx),
                receive(&mut recv, expect_rx, &tx, &budget),
            )
        } => Some(result),
        () = work.stopped() => None,
    };
    match outcome {
        Some(Ok(_)) => Ok(()),
        // The sender abandoned the stream; it closes the connection right
        // after, and the close says why.
        Some(Err(_)) if recv.reset_by_peer() => Err(TransportError::closed_by_peer(&conn).await),
        Some(Err(e)) => Err(e),
        None => {
            let _ = send.reset(STREAM_CANCELLED);
            let _ = recv.stream_mut().stop(STREAM_CANCELLED);
            Ok(())
        }
    }
}

/// Requested batch plus its share of the stream's outstanding window.
type Expected = (Vec<Job>, OwnedSemaphorePermit);

async fn request(
    send: &mut quinn::SendStream,
    work: &Work,
    first: Vec<Job>,
    expect: mpsc::UnboundedSender<Expected>,
) -> Result<()> {
    send.write_all(&encode_frame(&StreamOpen::Chunks)?)
        .await
        .map_err(|e| TransportError::write(&e))?;
    let window = Arc::new(Semaphore::new(STREAM_OUTSTANDING_BYTES));
    let mut batch = Some(first);
    while let Some(jobs) = batch {
        let bytes: usize = jobs.iter().map(|j| j.chunk.len as usize).sum();
        let permit = Arc::clone(&window)
            .acquire_many_owned(budget_permits(bytes, STREAM_OUTSTANDING_BYTES))
            .await
            .map_err(|e| TransportError::internal("request window", e))?;
        let frame = encode_frame(&ChunkRequest {
            hashes: jobs.iter().map(|j| j.chunk.hash).collect(),
        })?;
        // Expect before sending: the reader must know the order first.
        if expect.send((jobs, permit)).is_err() {
            return Ok(());
        }
        send.write_all(&frame)
            .await
            .map_err(|e| TransportError::write(&e))?;
        batch = work.next_batch().await;
    }
    let _ = send.finish();
    Ok(())
}

async fn receive(
    recv: &mut StreamReader,
    mut expect: mpsc::UnboundedReceiver<Expected>,
    tx: &mpsc::UnboundedSender<Delivery>,
    budget: &Arc<Semaphore>,
) -> Result<()> {
    while let Some((jobs, _window)) = expect.recv().await {
        for job in jobs {
            let header = recv
                .record_header()
                .await?
                .ok_or_else(|| TransportError::from(ProtocolViolation::ChunkStreamEnded))?;
            if header.hash != job.chunk.hash {
                return Err(ProtocolViolation::UnrequestedChunk.into());
            }
            let delivery = match header.kind {
                RecordKind::Unavailable => Delivery::Unavailable { job },
                RecordKind::Data => {
                    if header.len != job.chunk.len {
                        return Err(ProtocolViolation::ChunkLengthMismatch.into());
                    }
                    let len = chunk_len(job.chunk.len)?;
                    let permit = Arc::clone(budget)
                        .acquire_many_owned(budget_permits(len, DELIVERY_BUDGET_BYTES))
                        .await
                        .map_err(|e| TransportError::internal("delivery budget", e))?;
                    Delivery::Data {
                        job,
                        bytes: recv.payload(len).await?,
                        budget: permit,
                    }
                }
            };
            if tx.send(delivery).is_err() {
                // The caller stopped consuming (abort).
                return Ok(());
            }
        }
    }
    Ok(())
}
