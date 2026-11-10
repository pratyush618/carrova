//! The chunk pull of a LAN import: batches fetched from the sender into
//! the staging store, with checkpoints, disk-space pauses and reconnects.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use carrova_manifest::{ChunkHash, ChunkRef, Manifest};
use carrova_platform::Platform;
use carrova_transport::{DEFAULT_PARALLEL_FETCHES, FailedChunk, FetchAbort, TransportError};
use zeroize::Zeroizing;

use crate::cancel::Cancel;
use crate::error::{CoreError, Result};
use crate::key::ItemKey;
use crate::lan::{
    self, LanMsg, Link, RECONNECT_WINDOW, ReceiverStage, SPACE_POLL_INTERVAL, TRANSFER_BATCH_BYTES,
};
use crate::plan::Plan;
use crate::session::progress::PROGRESS_STEP;
use crate::session::{Event, EventSink, PauseReason, Phase, ProgressMeter};
use crate::stage::{CheckpointPolicy, StageLayout, Staged, StagingStore};
use crate::transfer_control::{IDLE_SLICE, TransferControl};

/// One transfer run: the staging store being filled and its progress.
pub(super) struct Transfer<'a> {
    disk: Disk<'a>,
    store: StagingStore,
    progress: ProgressMeter<'a>,
}

impl<'a> Transfer<'a> {
    /// Open (or reopen, keeping checkpointed chunks) the staging store for
    /// `plan`, waiting for disk space if needed. Returns the run and the
    /// chunks of `order` still to fetch, which its progress counts (a
    /// resumed transfer counts only what is left).
    pub(super) fn open(
        disk: Disk<'a>,
        plan: &Plan,
        manifest: &Manifest,
        order: &[ChunkRef],
    ) -> Result<(Self, Vec<ChunkRef>)> {
        let store = disk.wait(|| {
            let layout = StageLayout::new(plan, manifest)?;
            StagingStore::open(disk.platform, disk.dir, layout, CheckpointPolicy::Periodic)
        })?;
        let queue = store.missing(order);
        let progress =
            ProgressMeter::new(disk.sink, Phase::Transfer, bytes_of(&queue), PROGRESS_STEP).start();
        let run = Self {
            disk,
            store,
            progress,
        };
        Ok((run, queue))
    }

    /// The chunks of `chunks` not staged yet, minus those the sender gave
    /// up on (`failed`), in order.
    pub(super) fn missing(&self, chunks: &[ChunkRef], failed: &[FailedChunk]) -> Vec<ChunkRef> {
        let given_up: HashSet<ChunkHash> = failed.iter().map(|f| f.chunk.hash).collect();
        self.store
            .missing(chunks)
            .into_iter()
            .filter(|c| !given_up.contains(&c.hash))
            .collect()
    }

    /// Fetch `batch` into staging and checkpoint: [`lan::check_space`]
    /// first, a lost connection is re-established on the next batch, a
    /// full disk pauses until space frees up. Returns the chunks the
    /// sender could not supply.
    pub(super) fn fetch_batch(
        &mut self,
        link: &mut Link,
        batch: &[ChunkRef],
    ) -> Result<Vec<FailedChunk>> {
        let sink = self.disk.sink;
        self.disk.wait(|| self.disk.check_space(batch))?;
        let platform = self.disk.platform;
        let conn = link.connected(ReceiverStage::Transferring, RECONNECT_WINDOW, sink)?;
        let mut refused: Option<CoreError> = None;
        let (store, progress) = (&mut self.store, &mut self.progress);
        let fetched = conn.send(&LanMsg::Fetch).and_then(|()| {
            conn.fetch_chunks(batch, DEFAULT_PARALLEL_FETCHES, &mut |chunk, bytes| {
                let bytes = Zeroizing::new(bytes);
                store.put(platform, chunk, &bytes).map_err(|e| {
                    let abort = if matches!(e, CoreError::DiskFull { .. }) {
                        FetchAbort::OutOfSpace
                    } else {
                        FetchAbort::Failed(e.to_string().into())
                    };
                    refused = Some(e);
                    abort
                })?;
                progress.add(u64::from(chunk.len));
                Ok(())
            })
        });
        let failed = match fetched {
            Ok(report) => report.failed,
            Err(TransportError::Aborted(_)) => match refused {
                Some(CoreError::DiskFull { needed, .. }) => {
                    self.checkpoint()?;
                    self.disk.wait(|| self.disk.free_at_least(needed))?;
                    Vec::new()
                }
                Some(e) => return Err(e),
                None => return Err(CoreError::FetchAborted),
            },
            Err(e) if e.is_lost() => {
                self.checkpoint()?;
                link.lost(sink);
                Vec::new()
            }
            Err(e @ (TransportError::Cancelled | TransportError::PeerCancelled)) => {
                // Keep what arrived for a resume.
                self.checkpoint()?;
                return Err(e.into());
            }
            Err(e) => return Err(e.into()),
        };
        self.checkpoint()?;
        Ok(failed)
    }

    fn checkpoint(&mut self) -> Result<()> {
        let platform = self.disk.platform;
        let store = &mut self.store;
        self.disk.wait(|| store.checkpoint(platform))
    }

    /// End the run: verified staging, or [`CoreError::TransferIncomplete`]
    /// naming the items of the chunks the sender could not supply.
    pub(super) fn finish(mut self, failed: &[FailedChunk]) -> Result<Staged> {
        tracing::info!(
            fetched = self.progress.done(),
            needed = self.progress.total(),
            "transfer finished"
        );
        self.progress.finish();
        if !failed.is_empty() {
            let layout = self.store.layout();
            let items: BTreeSet<ItemKey> = failed
                .iter()
                .flat_map(|f| layout.places(&f.chunk.hash))
                .filter_map(|p| layout.files().get(p.file))
                .map(|f| f.target.item().clone())
                .collect();
            let items: Vec<ItemKey> = items.into_iter().collect();
            tracing::warn!(items = items.len(), "sender could not supply some items");
            return Err(CoreError::TransferIncomplete { items });
        }
        self.store.finish(self.disk.platform)
    }
}

/// Sum of chunk lengths.
fn bytes_of(chunks: &[ChunkRef]) -> u64 {
    chunks.iter().map(|c| u64::from(c.len)).sum()
}

/// The longest prefix of `queue` within [`TRANSFER_BATCH_BYTES`] (at
/// least one chunk).
pub(super) fn next_batch(queue: &[ChunkRef]) -> &[ChunkRef] {
    let mut bytes = 0u64;
    let n = queue
        .iter()
        .take_while(|c| {
            bytes += u64::from(c.len);
            bytes <= TRANSFER_BATCH_BYTES
        })
        .count();
    &queue[..n.max(1)]
}

/// Disk-space waits of one transfer: retry an operation that reports
/// [`CoreError::DiskFull`] every [`SPACE_POLL_INTERVAL`], announcing the
/// pause and the resumption, until a cancel. The free-space guard
/// ([`Self::check_space`]) is skipped once the user overrides it
/// ([`TransferControl::ignore_free_space`]); a real full disk still waits.
pub(super) struct Disk<'a> {
    platform: &'a dyn Platform,
    dir: &'a Path,
    sink: &'a dyn EventSink,
    cancel: &'a Cancel,
    control: &'a TransferControl,
    /// Shown in the pause reason (Carrova's state directory).
    label: String,
}

impl<'a> Disk<'a> {
    /// Waits on the filesystem of `dir`; pauses name `state_root`
    /// (Carrova's state directory); `cancel` ends a wait, `control` may
    /// lift the free-space guard.
    pub(super) fn new(
        platform: &'a dyn Platform,
        dir: &'a Path,
        sink: &'a dyn EventSink,
        state_root: &Path,
        cancel: &'a Cancel,
        control: &'a TransferControl,
    ) -> Self {
        Self {
            platform,
            dir,
            sink,
            cancel,
            control,
            label: state_root.display().to_string(),
        }
    }

    /// Run `op` until it does not fail with [`CoreError::DiskFull`].
    pub(super) fn wait<T>(&self, mut op: impl FnMut() -> Result<T>) -> Result<T> {
        let mut paused = false;
        loop {
            match op() {
                Err(CoreError::DiskFull { needed, available }) => {
                    if !paused {
                        paused = true;
                        tracing::info!(needed, available, "transfer paused: low disk space");
                        self.sink.emit(Event::TransferPaused {
                            reason: PauseReason::LowDiskSpace {
                                fs: self.label.clone(),
                                available,
                                needed,
                                disk: self.mount_point(),
                            },
                        });
                    }
                    self.idle()?;
                }
                other => {
                    if paused {
                        tracing::info!("transfer resumed");
                        self.sink.emit(Event::TransferResumed);
                    }
                    return other;
                }
            }
        }
    }

    /// Mount point of the waited-on filesystem, if the platform knows it.
    /// Asks about the nearest existing ancestor: staging may not exist yet
    /// (first transfer) or was wiped after a failed preallocation.
    fn mount_point(&self) -> Option<String> {
        let existing = crate::space::nearest_existing(self.platform, self.dir)?;
        let disk = self.platform.disk(&existing).ok()?;
        Some(disk.mount_point.display().to_string())
    }

    /// Wait [`SPACE_POLL_INTERVAL`] before the next retry, ending early
    /// once the user lifts the free-space guard.
    fn idle(&self) -> Result<()> {
        // At least one slice: with the guard lifted, a really full disk
        // would otherwise retry in a hot loop.
        self.cancel.sleep(IDLE_SLICE)?;
        let mut waited = IDLE_SLICE;
        while waited < SPACE_POLL_INTERVAL && !self.control.ignores_free_space() {
            self.cancel.sleep(IDLE_SLICE)?;
            waited += IDLE_SLICE;
        }
        Ok(())
    }

    /// The free-space guard before fetching `batch`
    /// ([`lan::check_space`]), unless the user overrode it.
    fn check_space(&self, batch: &[ChunkRef]) -> Result<()> {
        if self.control.ignores_free_space() {
            return Ok(());
        }
        lan::check_space(self.platform, self.dir, batch)
    }

    /// At least `needed` bytes free on the staging filesystem.
    fn free_at_least(&self, needed: u64) -> Result<()> {
        let available = self.platform.free_space(self.dir)?;
        if available < needed {
            return Err(CoreError::DiskFull { needed, available });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/engine/transfer/unit.rs"]
mod tests;
