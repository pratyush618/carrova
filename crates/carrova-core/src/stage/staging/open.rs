//! Opening a [`StagingStore`]: fresh staging, or resuming staging that
//! matches the layout.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

use carrova_manifest::ChunkHash;
use carrova_platform::{Allocation, FileKind, Platform};

use super::{CheckpointPolicy, DIR_MODE, FILES_DIR, PROGRESS_LOG, StagingStore};
use crate::error::Result;
use crate::stage::layout::{STAGED_MODE, StageLayout};
use crate::stage::progress_log::{HEADER_LEN, header, header_matches, parse_records};
use crate::stage::{session_dir, wipe};
use crate::{fsx, space};

impl StagingStore {
    /// Open the staging area `staging_dir` (its parent is the session dir)
    /// for `layout`.
    ///
    /// Resumes when `progress.log` names the same layout fingerprint and
    /// every staged file is present at its size: checkpointed chunks are
    /// kept, a torn log tail is dropped. Otherwise any stale staging is
    /// wiped and every `files/<n>` is created (`600`) and preallocated to
    /// its final size; a full disk is [`CoreError::DiskFull`] (with the
    /// partial staging wiped again so the space is free while waiting).
    pub fn open(
        platform: &dyn Platform,
        staging_dir: &Path,
        layout: StageLayout,
        policy: CheckpointPolicy,
    ) -> Result<Self> {
        let session_dir = session_dir(staging_dir)?;
        if let Some(r) = Self::resume(platform, staging_dir, &layout)? {
            tracing::debug!(
                verified = r.verified.len(),
                needed = layout.unique_chunk_count(),
                "resuming staging"
            );
            return Ok(Self::with(staging_dir, layout, policy, r.verified, r.log));
        }
        wipe(platform, staging_dir, session_dir)?;
        match Self::create(platform, staging_dir, &layout) {
            Ok(log) => Ok(Self::with(staging_dir, layout, policy, HashSet::new(), log)),
            Err(e) => {
                if let Err(w) = wipe(platform, staging_dir, session_dir) {
                    tracing::warn!(error = %w, "could not wipe partial staging");
                }
                Err(e)
            }
        }
    }

    fn with(
        staging_dir: &Path,
        layout: StageLayout,
        policy: CheckpointPolicy,
        verified: HashSet<ChunkHash>,
        log: File,
    ) -> Self {
        Self {
            dir: staging_dir.to_path_buf(),
            files_dir: staging_dir.join(FILES_DIR),
            layout,
            policy,
            verified,
            pending: Vec::new(),
            pending_set: HashSet::new(),
            pending_bytes: 0,
            dirty: BTreeSet::new(),
            open: HashMap::new(),
            log,
            last_checkpoint: Instant::now(),
        }
    }

    /// Fresh staging: dirs, pre-sized files, then the log header (written
    /// last, so a header means every file exists at full size). Where the
    /// filesystem cannot reserve blocks the files are sparse, and a full
    /// disk surfaces later as [`CoreError::DiskFull`] from [`Self::put`].
    fn create(platform: &dyn Platform, staging_dir: &Path, layout: &StageLayout) -> Result<File> {
        let files_dir = staging_dir.join(FILES_DIR);
        platform.create_dir(staging_dir, Some(DIR_MODE))?;
        platform.create_dir(&files_dir, Some(DIR_MODE))?;
        let mut left = layout.total_bytes();
        for (n, spec) in layout.files().iter().enumerate() {
            let f = platform.create_new_file(&files_dir.join(n.to_string()), Some(STAGED_MODE))?;
            match platform.preallocate(&f, spec.size) {
                Ok(Allocation::Reserved) => {}
                Ok(Allocation::Sparse) => {
                    tracing::debug!("filesystem cannot preallocate; staged files are sparse");
                }
                Err(e) => return Err(space::disk_full_or(platform, staging_dir, e, left)),
            }
            left = left.saturating_sub(spec.size);
        }
        platform.sync_dir(&files_dir)?;
        let mut log =
            platform.create_new_file(&staging_dir.join(PROGRESS_LOG), Some(STAGED_MODE))?;
        log.write_all(&header(layout.fingerprint()))?;
        log.sync_data()?;
        platform.sync_dir(staging_dir)?;
        Ok(log)
    }

    /// Reuse existing staging for `layout`, or `None` if there is none or
    /// it belongs to another layout / is damaged.
    fn resume(
        platform: &dyn Platform,
        staging_dir: &Path,
        layout: &StageLayout,
    ) -> Result<Option<Resumed>> {
        let log_path = staging_dir.join(PROGRESS_LOG);
        // Missing, or not a regular file (e.g. a symlink): restage.
        let Ok(mut f) = platform.open_read_nofollow(&log_path) else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes)?;
        drop(f);
        if !header_matches(&bytes, layout.fingerprint()) {
            return Ok(None);
        }
        let files_dir = staging_dir.join(FILES_DIR);
        for (n, spec) in layout.files().iter().enumerate() {
            match fsx::lstat(platform, &files_dir.join(n.to_string()))? {
                Some(m) if m.kind == FileKind::File && m.size == spec.size => {}
                _ => return Ok(None),
            }
        }
        let (records, valid_len) = parse_records(&bytes[HEADER_LEN..]);
        let mut verified = HashSet::new();
        for h in records {
            if layout.chunk(&h).is_none() {
                return Ok(None);
            }
            verified.insert(h);
        }
        let valid_len = (HEADER_LEN + valid_len) as u64;
        let log = platform.open_append_nofollow(&log_path)?;
        if valid_len < bytes.len() as u64 {
            tracing::debug!(
                dropped = bytes.len() as u64 - valid_len,
                "dropping torn progress log tail"
            );
            log.set_len(valid_len)?;
            log.sync_data()?;
        }
        Ok(Some(Resumed { log, verified }))
    }
}

/// Existing staging that matches the layout.
struct Resumed {
    log: File,
    verified: HashSet<ChunkHash>,
}
