//! Making staged data durable ([`StagingStore::checkpoint`]) and
//! checking it complete ([`StagingStore::finish`]).

use std::io::{Read, Write};
use std::time::Instant;

use carrova_manifest::{Blake3Hash, ChunkRef};
use carrova_platform::{Platform, PlatformError};
use zeroize::Zeroizing;

use super::{SYNC_FS_MIN_FILES, StagingStore, VERIFY_BUF, handle};
use crate::error::{CoreError, Result};
use crate::space;
use crate::stage::Staged;
use crate::stage::layout::StageTarget;
use crate::stage::progress_log::record;

impl StagingStore {
    /// Make everything written so far durable: sync the data (one
    /// `Platform::sync_fs` for many written files, else — or where that is
    /// unsupported — `sync_data` per written file), then append the chunks to `progress.log` and sync it.
    /// Staged file entries never change after [`Self::open`] (which synced
    /// `files/`), so no directory sync is needed here. No-op when nothing
    /// was written.
    pub fn checkpoint(&mut self, platform: &dyn Platform) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let synced_fs = self.dirty.len() >= SYNC_FS_MIN_FILES
            && match platform.sync_fs(&self.files_dir) {
                Ok(()) => true,
                Err(PlatformError::Unsupported(_)) => false,
                Err(e) => return Err(e.into()),
            };
        if !synced_fs {
            for n in std::mem::take(&mut self.dirty) {
                handle(platform, &mut self.open, &self.files_dir, n)?.sync_data()?;
            }
        }
        let record = record(&self.pending);
        let written = self
            .log
            .write_all(&record)
            .and_then(|()| self.log.sync_data());
        if let Err(e) = written {
            let needed = record.len() as u64;
            return Err(space::disk_full_or(platform, &self.dir, e, needed));
        }
        self.verified.extend(self.pending.drain(..));
        self.pending_set.clear();
        self.dirty.clear();
        self.pending_bytes = 0;
        self.last_checkpoint = Instant::now();
        Ok(())
    }

    /// Final checkpoint, then check that every needed chunk arrived and
    /// every staged file has its manifest size and whole-file BLAKE3.
    /// Returns the map apply consumes (files stay `600` in `files/`).
    /// Any mismatch is [`CoreError::Verification`].
    pub fn finish(&mut self, platform: &dyn Platform) -> Result<Staged> {
        self.checkpoint(platform)?;
        self.open.clear();
        let all: Vec<ChunkRef> = self.layout.unique_chunks().collect();
        let absent = self.missing(&all).len();
        if absent > 0 {
            return Err(CoreError::Verification(format!(
                "{absent} needed chunk(s) never arrived"
            )));
        }
        let mut staged = Staged::default();
        let mut buf = Zeroizing::new(vec![0u8; VERIFY_BUF]);
        for (n, spec) in self.layout.files().iter().enumerate() {
            let path = self.files_dir.join(n.to_string());
            let bad = || CoreError::Verification(spec.target.label());
            let mut f = platform.open_read_nofollow(&path)?;
            let mut hasher = blake3::Hasher::new();
            let mut size = 0u64;
            loop {
                let k = f.read(&mut buf)?;
                if k == 0 {
                    break;
                }
                size = size.saturating_add(k as u64);
                hasher.update(buf.get(..k).ok_or_else(bad)?);
            }
            if size != spec.size || Blake3Hash::from(hasher.finalize()) != spec.hash {
                return Err(bad());
            }
            match &spec.target {
                StageTarget::File { item, entry_index } => {
                    staged.files.insert((item.clone(), *entry_index), path);
                }
                StageTarget::Artifact(id) => {
                    staged.artifacts.insert(id.clone(), path);
                }
            }
        }
        Ok(staged)
    }
}
