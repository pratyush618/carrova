//! Sender side of chunk transfer: answer requests from a [`ChunkSource`].
//!
//! Disk reads run on the caller's thread (the source is not `'static`, and
//! blocking I/O never touches tokio workers). Stream tasks queue reads as
//! soon as requests arrive, so the next batch is read while the current
//! one is on the wire; read-but-unsent bytes per stream are bounded by
//! [`SERVE_STREAM_BUDGET_BYTES`]. Small chunk records are coalesced into
//! writes of up to [`RESPONSE_COALESCE_BYTES`].
//!
//! A cancel (checked before every read, polled by the driver while idle)
//! resets the chunk streams; the caller then closes the connection as
//! cancelled so the receiver learns why.
//!
//! [`SERVE_STREAM_BUDGET_BYTES`]: crate::tuning::SERVE_STREAM_BUDGET_BYTES
//! [`RESPONSE_COALESCE_BYTES`]: crate::tuning::RESPONSE_COALESCE_BYTES

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use carrova_manifest::{ChunkRef, ChunkSource};
use quinn::SendStream;
use tokio::sync::{mpsc, oneshot};

use crate::cancel::Watch;
use crate::connection::{AllowedChunks, ServeEnd};
use crate::error::{Result, TransportError};
use crate::runtime::{Driver, Stop, Stoppable};
use crate::tuning::{SERVE_READ_QUEUE, SERVED_BYTES_FACTOR};

mod drive;
mod outbox;
mod quota;
mod stream;

use drive::drive;
use quota::Quota;

/// A disk read for the caller's thread. `None` = chunk unavailable.
struct ReadJob {
    chunk: ChunkRef,
    reply: oneshot::Sender<Option<Vec<u8>>>,
}

/// Serve chunks over `conn` (see [`crate::Connection::serve_chunks`]).
/// `requested` counts chunk bytes requested over the whole connection.
pub(crate) fn serve(
    conn: &quinn::Connection,
    allowed: &AllowedChunks,
    source: &mut dyn ChunkSource,
    requested: &Arc<AtomicU64>,
    cancel: &Watch,
) -> Result<ServeEnd> {
    let (jobs_tx, mut jobs_rx) = mpsc::channel(SERVE_READ_QUEUE);
    let stop = Arc::new(Stop::default());
    let quota = Quota {
        requested: Arc::clone(requested),
        ceiling: allowed.unique_bytes().saturating_mul(SERVED_BYTES_FACTOR),
    };
    // Declared after `jobs_rx`: on unwind the driver is stopped and joined
    // (streams reset) before queued reads are dropped.
    let driver = Driver::spawn(
        drive(
            conn.clone(),
            allowed.clone(),
            quota,
            jobs_tx,
            Arc::clone(&stop),
        ),
        Arc::clone(&stop) as Arc<dyn Stoppable>,
        cancel.clone(),
    )?;
    while let Some(job) = jobs_rx.blocking_recv() {
        // Between chunks; an idle serve is stopped by the driver's watch.
        if cancel.is_cancelled() {
            stop.stop();
            break;
        }
        // The requesting stream may be gone (cancelled fetch): fine.
        let _ = job.reply.send(read(source, &job.chunk));
    }
    // Dropping queued reads lets the streams see the stop.
    drop(jobs_rx);
    let served = driver.join();
    if cancel.is_cancelled() {
        return Err(TransportError::Cancelled);
    }
    served
}

fn read(source: &mut dyn ChunkSource, chunk: &ChunkRef) -> Option<Vec<u8>> {
    match source.read_chunk(chunk) {
        Ok(bytes) if bytes.len() == chunk.len as usize => Some(bytes),
        Ok(_) => {
            tracing::debug!(chunk = %chunk.hash, "chunk source returned the wrong length");
            None
        }
        Err(e) => {
            tracing::debug!(chunk = %chunk.hash, error = %e, "chunk unavailable");
            None
        }
    }
}

/// How one receiver-opened stream ended.
enum StreamEnd {
    /// A chunk stream ended (finished or cancelled by the receiver).
    Served(ServeEnd),
    /// The receiver's fetch-done stream; answered once all others drained.
    Done(SendStream),
}

fn add(total: &mut ServeEnd, s: ServeEnd) {
    total.chunks += s.chunks;
    total.bytes += s.bytes;
}
