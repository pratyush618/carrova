//! [`StagingStore`]: chunks written straight into pre-sized staged files,
//! with crash-safe progress for resumable (LAN) transfers. Opening
//! (fresh or resumed) lives in `open`, checkpoints and the final check in
//! `durable`.
//!
//! Layout of `staging/` (dir `700`, files `600`):
//!
//! ```text
//! files/<n>       staged file n of the StageLayout, pre-sized to its final size
//! progress.log    header (layout fingerprint) + checkpoint records
//! ```
//!
//! `progress.log` = header `MAGIC | u16 version | u16 0 | u32 0 |
//! fingerprint(32) | sum(8)`, then one record per checkpoint:
//! `u32 count | count × chunk hash(32) | sum(8)` (little-endian), where
//! `sum` is the first 8 bytes of BLAKE3 over the preceding bytes of the
//! frame. A torn or garbled tail is dropped (and truncated away) on open,
//! so a crash leaves each chunk either recorded or re-fetched.
//!
//! A chunk counts as verified (see [`StagingStore::missing`]) only once
//! its record is durable, and a record is appended only after the data it
//! covers has been synced.

use std::collections::hash_map::Entry;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use carrova_manifest::{Blake3Hash, ChunkHash, ChunkRef};
use carrova_platform::Platform;

use super::layout::StageLayout;
pub use super::progress_log::LOG_BYTES_PER_CHUNK;
use crate::error::{CoreError, Result};
use crate::space;

mod durable;
mod open;

/// Staged files directory inside `staging/`.
pub const FILES_DIR: &str = "files";
/// Checkpoint log inside `staging/`.
pub const PROGRESS_LOG: &str = "progress.log";

/// [`CheckpointPolicy::Periodic`]: checkpoint after this many bytes were
/// written since the last one (bounds what a killed receiver re-fetches)…
pub const CHECKPOINT_BYTES: u64 = 16 * 1024 * 1024;
/// …or once this much time has passed since the last one.
pub const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(2);

/// A checkpoint with at least this many written files syncs them with one
/// `Platform::sync_fs`; fewer get `sync_data` each, which does not wait
/// for unrelated dirty data on the same filesystem (`syncfs` flushes all
/// of it).
const SYNC_FS_MIN_FILES: usize = 64;
/// Staged file handles kept open between writes (bounds fd use for plans
/// with many files; chunks of one file mostly arrive together).
const OPEN_FILES_MAX: usize = 64;
/// Read buffer for the whole-file check in [`StagingStore::finish`].
const VERIFY_BUF: usize = 1024 * 1024;

const DIR_MODE: u32 = 0o700;

/// When [`StagingStore`] makes written chunks durable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointPolicy {
    /// Every [`CHECKPOINT_BYTES`] / [`CHECKPOINT_INTERVAL`] (and on
    /// [`StagingStore::checkpoint`] / [`StagingStore::finish`]): LAN
    /// transfers resume from the last checkpoint.
    Periodic,
    /// Only in [`StagingStore::finish`]: archive staging restarts from
    /// scratch after any interruption anyway.
    OnFinish,
}

/// Receiver staging area for one [`StageLayout`] (see the module docs).
///
/// [`Self::put`] verifies a chunk (length + BLAKE3) once and writes it at
/// every place the layout lists; [`Self::finish`] checks every staged file
/// (size + whole-file BLAKE3) and hands out the [`Staged`] map apply
/// consumes.
#[derive(Debug)]
pub struct StagingStore {
    dir: PathBuf,
    files_dir: PathBuf,
    layout: StageLayout,
    policy: CheckpointPolicy,
    /// Chunks whose record is durable in `progress.log`.
    verified: HashSet<ChunkHash>,
    /// Written since the last checkpoint, in write order.
    pending: Vec<ChunkHash>,
    pending_set: HashSet<ChunkHash>,
    pending_bytes: u64,
    /// Staged files written since the last checkpoint.
    dirty: BTreeSet<usize>,
    open: HashMap<usize, File>,
    log: File,
    last_checkpoint: Instant,
}

impl StagingStore {
    /// The layout this store stages.
    #[must_use]
    pub fn layout(&self) -> &StageLayout {
        &self.layout
    }

    /// Verify `bytes` against `chunk` (the layout's length + BLAKE3) and
    /// write them at every place the layout lists. A chunk already written
    /// is a no-op; a chunk the layout does not need or a mismatch is
    /// [`CoreError::Verification`] and writes nothing. With
    /// [`CheckpointPolicy::Periodic`] this may run [`Self::checkpoint`].
    pub fn put(&mut self, platform: &dyn Platform, chunk: &ChunkRef, bytes: &[u8]) -> Result<()> {
        let hash = chunk.hash;
        let Some(entry) = self.layout.chunk(&hash) else {
            return Err(CoreError::Verification(format!(
                "chunk {hash} is not needed by the plan"
            )));
        };
        if chunk.len != entry.len
            || bytes.len() != entry.len as usize
            || Blake3Hash::of(bytes) != hash
        {
            return Err(CoreError::Verification(format!("chunk {hash}")));
        }
        if self.contains(&hash) || self.pending_set.contains(&hash) {
            return Ok(());
        }
        for p in &entry.places {
            let file = handle(platform, &mut self.open, &self.files_dir, p.file)?;
            if let Err(e) = platform.write_all_at(file, bytes, p.offset) {
                let needed = self.remaining_bytes();
                return Err(space::disk_full_or(platform, &self.dir, e, needed));
            }
            self.dirty.insert(p.file);
        }
        let written = u64::from(entry.len).saturating_mul(entry.places.len() as u64);
        self.pending.push(hash);
        self.pending_set.insert(hash);
        self.pending_bytes = self.pending_bytes.saturating_add(written);
        if self.policy == CheckpointPolicy::Periodic
            && (self.pending_bytes >= CHECKPOINT_BYTES
                || self.last_checkpoint.elapsed() >= CHECKPOINT_INTERVAL)
        {
            self.checkpoint(platform)?;
        }
        Ok(())
    }

    /// Chunk `hash` is checkpointed (durably staged).
    #[must_use]
    pub fn contains(&self, hash: &ChunkHash) -> bool {
        self.verified.contains(hash)
    }

    /// The chunks of `chunks` not checkpointed yet, deduplicated, order
    /// preserved (filter [`crate::lan::transfer_order`] with this).
    #[must_use]
    pub fn missing(&self, chunks: &[ChunkRef]) -> Vec<ChunkRef> {
        let mut seen = HashSet::new();
        chunks
            .iter()
            .filter(|c| !self.verified.contains(&c.hash) && seen.insert(c.hash))
            .copied()
            .collect()
    }

    /// Bytes still to be written (unverified chunks × their places).
    fn remaining_bytes(&self) -> u64 {
        self.layout
            .unique_chunks()
            .filter(|c| !self.verified.contains(&c.hash) && !self.pending_set.contains(&c.hash))
            .map(|c| u64::from(c.len) * self.layout.places(&c.hash).len() as u64)
            .fold(0u64, u64::saturating_add)
    }
}

/// Open handle of staged file `n` from the bounded cache `open` (cleared
/// when full; chunks of one file mostly arrive together).
fn handle<'a>(
    platform: &dyn Platform,
    open: &'a mut HashMap<usize, File>,
    files_dir: &Path,
    n: usize,
) -> Result<&'a File> {
    if !open.contains_key(&n) && open.len() >= OPEN_FILES_MAX {
        open.clear();
    }
    match open.entry(n) {
        Entry::Occupied(e) => Ok(e.into_mut()),
        Entry::Vacant(v) => {
            let f = platform.open_rw_nofollow(&files_dir.join(n.to_string()))?;
            Ok(v.insert(f))
        }
    }
}
