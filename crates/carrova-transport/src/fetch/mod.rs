//! Receiver side of chunk transfer: pull, verify, deliver.
//!
//! - A shared queue holds the chunks in priority order; each chunk stream
//!   takes ~[`REQUEST_BATCH_BYTES`] batches from its front, keeping up to
//!   [`STREAM_OUTSTANDING_BYTES`] requested per stream (≥ 2 batches in
//!   flight) so the sender never idles between batches.
//! - Records are read on the runtime and handed to the caller's thread
//!   through a channel bounded by [`DELIVERY_BUDGET_BYTES`]; BLAKE3
//!   verification and the chunk callback run there, never on tokio workers.
//! - A chunk failing verification goes back to the queue front, up to
//!   [`MAX_CHUNK_RETRIES`] times.
//! - A cancel (checked between deliveries, polled by the driver while
//!   idle) stops the streams like a callback abort: the fetch-done
//!   exchange still runs, so the sender's serve ends normally.
//!
//! [`REQUEST_BATCH_BYTES`]: crate::tuning::REQUEST_BATCH_BYTES
//! [`STREAM_OUTSTANDING_BYTES`]: crate::tuning::STREAM_OUTSTANDING_BYTES
//! [`DELIVERY_BUDGET_BYTES`]: crate::tuning::DELIVERY_BUDGET_BYTES
//! [`MAX_CHUNK_RETRIES`]: crate::MAX_CHUNK_RETRIES

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

use carrova_manifest::ChunkRef;
use tokio::sync::mpsc;

use crate::cancel::Watch;
use crate::connection::{FetchReport, OnChunk};
use crate::error::{Result, TransportError};
use crate::runtime::{Driver, Stoppable};
use crate::tuning::MAX_PARALLEL_FETCHES;

mod deliver;
mod stream;
mod work;

use deliver::deliver;
use stream::drive;
use work::{Job, Work};

/// Pull `chunks` over `conn` (see [`crate::Connection::fetch_chunks`]).
pub(crate) fn fetch(
    conn: &quinn::Connection,
    chunks: &[ChunkRef],
    parallel: usize,
    cancel: &Watch,
    on_chunk: &mut OnChunk<'_>,
) -> Result<FetchReport> {
    let mut seen = HashSet::new();
    let queue: VecDeque<Job> = chunks
        .iter()
        .filter(|c| seen.insert(c.hash))
        .map(|&chunk| Job { chunk, attempt: 0 })
        .collect();
    let work = Arc::new(Work::new(queue));
    let (tx, mut rx) = mpsc::unbounded_channel();
    // Declared after `rx`: on unwind the driver is stopped and joined
    // before undelivered records are dropped.
    let driver = Driver::spawn(
        drive(
            conn.clone(),
            Arc::clone(&work),
            tx,
            parallel.clamp(1, MAX_PARALLEL_FETCHES),
        ),
        Arc::clone(&work) as Arc<dyn Stoppable>,
        cancel.clone(),
    )?;

    let mut report = FetchReport::default();
    let mut abort = None;
    while let Some(delivery) = rx.blocking_recv() {
        if let Err(a) = deliver(delivery, &work, &mut report, on_chunk) {
            abort = Some(a);
            work.stop();
            break;
        }
        // Between chunks; an idle fetch is stopped by the driver's watch.
        if cancel.is_cancelled() {
            work.stop();
            break;
        }
    }
    // Dropping undelivered records releases their budget, unblocking
    // streams so they can see the stop.
    drop(rx);
    let driven = driver.join();
    match abort {
        Some(a) => Err(TransportError::Aborted(a)),
        None if cancel.is_cancelled() => Err(TransportError::Cancelled),
        None => driven.map(|()| report),
    }
}
