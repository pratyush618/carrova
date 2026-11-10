//! The shared fetch queue: batches for chunk streams, retries, stop.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use carrova_manifest::ChunkRef;
use tokio::sync::Notify;

use crate::runtime::Stoppable;
use crate::tuning::{REQUEST_BATCH_BYTES, REQUEST_BATCH_CHUNKS};

/// One chunk to fetch and how often it was already re-requested.
#[derive(Debug, Clone, Copy)]
pub(super) struct Job {
    pub(super) chunk: ChunkRef,
    pub(super) attempt: u32,
}

/// Work shared by all chunk streams and the caller's thread.
#[derive(Debug)]
pub(super) struct Work {
    state: Mutex<WorkState>,
    changed: Notify,
}

#[derive(Debug)]
struct WorkState {
    /// Chunks not yet requested, highest priority first.
    queue: VecDeque<Job>,
    /// Chunks not yet delivered or reported failed.
    unresolved: usize,
    /// Fetch cancelled (callback abort or a stream failed).
    stopped: bool,
}

enum Next {
    Batch(Vec<Job>),
    Wait,
    Finished,
}

impl Work {
    pub(super) fn new(queue: VecDeque<Job>) -> Self {
        Self {
            state: Mutex::new(WorkState {
                unresolved: queue.len(),
                queue,
                stopped: false,
            }),
            changed: Notify::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, WorkState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn poll_batch(&self) -> Next {
        let mut s = self.lock();
        if s.stopped || (s.queue.is_empty() && s.unresolved == 0) {
            return Next::Finished;
        }
        let mut batch = Vec::new();
        let mut bytes = 0u64;
        while bytes < REQUEST_BATCH_BYTES && batch.len() < REQUEST_BATCH_CHUNKS {
            let Some(job) = s.queue.pop_front() else {
                break;
            };
            bytes += u64::from(job.chunk.len);
            batch.push(job);
        }
        if batch.is_empty() {
            Next::Wait
        } else {
            Next::Batch(batch)
        }
    }

    /// Next batch for a stream; `None` once everything is resolved or the
    /// fetch stopped. Waits while chunks are in flight elsewhere (they may
    /// come back for a retry).
    pub(super) async fn next_batch(&self) -> Option<Vec<Job>> {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            match self.poll_batch() {
                Next::Batch(b) => return Some(b),
                Next::Finished => return None,
                Next::Wait => notified.await,
            }
        }
    }

    /// Resolves once the fetch is stopped.
    pub(super) async fn stopped(&self) {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.lock().stopped {
                return;
            }
            notified.await;
        }
    }

    pub(super) fn requeue(&self, job: Job) {
        self.lock().queue.push_front(job);
        self.changed.notify_waiters();
    }

    pub(super) fn resolve(&self) {
        let done = {
            let mut s = self.lock();
            s.unresolved = s.unresolved.saturating_sub(1);
            s.unresolved == 0
        };
        if done {
            self.changed.notify_waiters();
        }
    }
}

impl Stoppable for Work {
    fn stop(&self) {
        self.lock().stopped = true;
        self.changed.notify_waiters();
    }
}
