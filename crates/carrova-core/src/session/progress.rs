//! Byte progress reporting: one [`ProgressMeter`] per running phase,
//! emitting [`Event::Progress`] at most every `step` bytes, and
//! [`Metered`], a [`ChunkSource`] that feeds a meter as chunks are read.

use std::fmt;

use carrova_manifest::{ChunkRef, ChunkSource, ChunkSourceError};
use serde::{Deserialize, Serialize};

use super::{Event, EventSink};

/// Default reporting step of long byte streams (staging, LAN transfer and
/// send): one event per this many bytes.
pub const PROGRESS_STEP: u64 = 4 * 1024 * 1024;

/// What an [`Event::Progress`] measures. Serialised as its
/// [`Self::as_str`] name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Sender: reading and hashing the selected files.
    Capture,
    /// Sender: writing the archive file.
    Archive,
    /// Sender: serving chunks to a LAN receiver.
    Send,
    /// Receiver: fetching chunks from a LAN sender.
    Transfer,
    /// Receiver: staging an archive's data.
    Stage,
    /// Receiver: applying the plan.
    Apply,
}

impl Phase {
    /// Stable name (`capture`, `archive`, `send`, `transfer`, `stage`,
    /// `apply`), as serialised.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Archive => "archive",
            Self::Send => "send",
            Self::Transfer => "transfer",
            Self::Stage => "stage",
            Self::Apply => "apply",
        }
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Counts bytes of one phase and reports them on a sink.
pub struct ProgressMeter<'a> {
    sink: &'a dyn EventSink,
    phase: Phase,
    done: u64,
    total: u64,
    /// `done` at the last report.
    reported: u64,
    step: u64,
    /// `total` is an estimate: reports never show less than `done`.
    estimated: bool,
}

impl fmt::Debug for ProgressMeter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProgressMeter")
            .field("phase", &self.phase)
            .field("done", &self.done)
            .field("total", &self.total)
            .finish_non_exhaustive()
    }
}

impl<'a> ProgressMeter<'a> {
    /// Step that reports every update.
    pub const EVERY_UPDATE: u64 = 0;

    /// A meter at zero of `total` bytes, reporting every `step` bytes
    /// ([`Self::EVERY_UPDATE`] for every [`Self::add`]). Reports nothing
    /// yet (see [`Self::start`]).
    #[must_use]
    pub fn new(sink: &'a dyn EventSink, phase: Phase, total: u64, step: u64) -> Self {
        Self {
            sink,
            phase,
            done: 0,
            total,
            reported: 0,
            step,
            estimated: false,
        }
    }

    /// Continue from `done` bytes already counted (and reported) by an
    /// earlier run.
    #[must_use]
    pub fn resume(self, done: u64) -> Self {
        Self {
            done,
            reported: done,
            ..self
        }
    }

    /// Treat `total` as an estimate that the work may exceed (e.g. files
    /// that grew since they were sized): reports show `max(total, done)`.
    #[must_use]
    pub fn estimated(self) -> Self {
        Self {
            estimated: true,
            ..self
        }
    }

    /// Report the starting point.
    #[must_use]
    pub fn start(self) -> Self {
        self.report();
        self
    }

    /// Bytes counted so far.
    #[must_use]
    pub fn done(&self) -> u64 {
        self.done
    }

    /// Bytes the phase is expected to take.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Count `bytes`; report once a step's worth accumulated.
    pub fn add(&mut self, bytes: u64) {
        self.done = self.done.saturating_add(bytes);
        if self.done - self.reported >= self.step {
            self.reported = self.done;
            self.report();
        }
    }

    /// Report the current count (the end of a run).
    pub fn finish(&self) {
        self.report();
    }

    /// The phase completed: count it as `total` and report that.
    pub fn complete(&mut self) {
        self.done = self.total;
        self.reported = self.done;
        self.report();
    }

    fn report(&self) {
        let total = if self.estimated {
            self.total.max(self.done)
        } else {
            self.total
        };
        self.sink.emit(Event::Progress {
            phase: self.phase,
            done: self.done,
            total,
        });
    }
}

/// A [`ChunkSource`] that counts every chunk it reads on a
/// [`ProgressMeter`].
#[derive(Debug)]
pub struct Metered<'m, 'a, S> {
    inner: S,
    meter: &'m mut ProgressMeter<'a>,
}

impl<'m, 'a, S: ChunkSource> Metered<'m, 'a, S> {
    /// Wrap `inner`, counting on `meter`.
    pub fn new(inner: S, meter: &'m mut ProgressMeter<'a>) -> Self {
        Self { inner, meter }
    }
}

impl<S: ChunkSource> ChunkSource for Metered<'_, '_, S> {
    fn read_chunk(&mut self, chunk: &ChunkRef) -> Result<Vec<u8>, ChunkSourceError> {
        let bytes = self.inner.read_chunk(chunk)?;
        self.meter.add(bytes.len() as u64);
        Ok(bytes)
    }
}

#[cfg(test)]
#[path = "../../tests/session/progress/unit.rs"]
mod tests;
