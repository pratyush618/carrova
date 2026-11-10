//! Delivery on the caller's thread: verify, retry, hand to the callback.

use carrova_manifest::ChunkHash;
use tokio::sync::OwnedSemaphorePermit;

use super::work::{Job, Work};
use crate::MAX_CHUNK_RETRIES;
use crate::connection::{ChunkFailure, FailedChunk, FetchAbort, FetchReport, OnChunk};
use crate::error::Result;

/// A record handed to the caller's thread.
pub(super) enum Delivery {
    /// Bytes as received (not yet verified). The permit returns the bytes
    /// to the delivery budget once the callback is done with them.
    Data {
        job: Job,
        bytes: Vec<u8>,
        budget: OwnedSemaphorePermit,
    },
    /// The sender cannot supply the chunk.
    Unavailable { job: Job },
}

/// Verify one record on the caller's thread and hand it to `on_chunk`.
pub(super) fn deliver(
    delivery: Delivery,
    work: &Work,
    report: &mut FetchReport,
    on_chunk: &mut OnChunk<'_>,
) -> Result<(), FetchAbort> {
    match delivery {
        Delivery::Unavailable { job } => {
            report.failed.push(FailedChunk {
                chunk: job.chunk,
                reason: ChunkFailure::Unavailable,
            });
            work.resolve();
        }
        Delivery::Data { job, bytes, budget } => {
            if ChunkHash::of(&bytes) != job.chunk.hash {
                if job.attempt < MAX_CHUNK_RETRIES {
                    tracing::warn!(chunk = %job.chunk.hash, attempt = job.attempt + 1, "chunk failed verification; re-requesting");
                    work.requeue(Job {
                        chunk: job.chunk,
                        attempt: job.attempt + 1,
                    });
                } else {
                    tracing::warn!(chunk = %job.chunk.hash, "chunk failed verification on every attempt");
                    report.failed.push(FailedChunk {
                        chunk: job.chunk,
                        reason: ChunkFailure::VerificationFailed,
                    });
                    work.resolve();
                }
                return Ok(());
            }
            let len = bytes.len() as u64;
            on_chunk(&job.chunk, bytes)?;
            drop(budget);
            report.chunks += 1;
            report.bytes += len;
            work.resolve();
        }
    }
    Ok(())
}
