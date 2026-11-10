//! Staging (receiver side): write the data an approved plan needs into the
//! session's `staging/` directory and verify every file against the
//! manifest before anything touches the user's roots. Only data needed by
//! the plan (see [`StageLayout`]) is staged. Staging is wiped after
//! apply/rollback or on failure.
//!
//! Chunks are written straight into pre-sized staged files
//! (`staging/files/<n>`) by a [`StagingStore`], shared by archive and LAN
//! staging: every chunk is verified once on arrival and written at every
//! offset the layout lists; [`StagingStore::finish`] checks each whole
//! file. Apply then renames the staged files into place.

pub mod layout;
mod progress_log;
pub mod staging;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use carrova_archive::ArchiveReader;
use carrova_platform::{Platform, PlatformError};
use zeroize::Zeroizing;

use crate::cancel::Cancel;
use crate::error::{CoreError, Result};
use crate::key::{ArtifactId, ItemKey};
use crate::plan::Plan;
use crate::session::progress::PROGRESS_STEP;
use crate::session::{EventSink, Phase, ProgressMeter};

pub use layout::{STAGED_MODE, StageLayout};
pub use staging::{CheckpointPolicy, StagingStore};

/// Verified staged data.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Staged {
    /// `(item key, entry_index)` → staged file ([`STAGED_MODE`], verified hash).
    pub files: BTreeMap<(ItemKey, usize), PathBuf>,
    /// Handler artifact → staged artifact (verified hash).
    pub artifacts: BTreeMap<ArtifactId, PathBuf>,
}

/// Stage everything `plan` needs from `reader` into `staging_dir`
/// (created `700`; a stale one is wiped first, confined to its parent, the
/// session dir). Fails (and wipes `staging_dir`) on any error, including a
/// hash mismatch ([`CoreError::Verification`]).
///
/// Every archive chunk is read (`next_chunk` until `Ok(None)`): only then is
/// the archive proven complete, and only then are the staged files checked
/// and handed out. Needed chunks go straight into their staged files
/// ([`CheckpointPolicy::OnFinish`]: one data sync at the end; an
/// interrupted archive staging restarts from scratch). `cancel` is
/// checked before every chunk ([`CoreError::Cancelled`]).
pub fn stage(
    platform: &dyn Platform,
    reader: &mut ArchiveReader,
    plan: &Plan,
    staging_dir: &Path,
    sink: &dyn EventSink,
    cancel: &Cancel,
) -> Result<Staged> {
    let session_dir = session_dir(staging_dir)?;
    let result = stage_inner(
        platform,
        reader,
        plan,
        staging_dir,
        session_dir,
        sink,
        cancel,
    );
    if result.is_err()
        && let Err(e) = wipe(platform, staging_dir, session_dir)
    {
        tracing::warn!(error = %e, "could not wipe staging after failure");
    }
    result
}

fn stage_inner(
    platform: &dyn Platform,
    reader: &mut ArchiveReader,
    plan: &Plan,
    staging_dir: &Path,
    session_dir: &Path,
    sink: &dyn EventSink,
    cancel: &Cancel,
) -> Result<Staged> {
    let layout = StageLayout::new(plan, reader.manifest())?;
    let total = reader
        .manifest()
        .unique_chunk_refs()
        .fold(0u64, |acc, c| acc.saturating_add(u64::from(c.len)));

    wipe(platform, staging_dir, session_dir)?;
    let mut store = StagingStore::open(platform, staging_dir, layout, CheckpointPolicy::OnFinish)?;

    let mut progress = ProgressMeter::new(sink, Phase::Stage, total, PROGRESS_STEP).start();
    while let Some((hash, bytes)) = reader.next_chunk()? {
        cancel.check()?;
        let bytes = Zeroizing::new(bytes);
        if let Some(chunk) = store.layout().chunk_ref(&hash) {
            store.put(platform, &chunk, &bytes)?;
        }
        progress.add(bytes.len() as u64);
    }
    // Archive complete and authenticated: now check the staged files.
    let staged = store.finish(platform)?;
    progress.complete();
    Ok(staged)
}

/// The session directory a staging directory lives in (its parent).
fn session_dir(staging_dir: &Path) -> Result<&Path> {
    staging_dir.parent().ok_or_else(|| CoreError::UnsafePath {
        path: staging_dir.display().to_string(),
        reason: "staging dir has no parent".into(),
    })
}

/// Remove a staging directory (confined to the session dir). A missing
/// directory is fine.
pub fn wipe(platform: &dyn Platform, staging_dir: &Path, session_dir: &Path) -> Result<()> {
    match platform.remove_dir_all_confined(staging_dir, session_dir) {
        Ok(()) => Ok(()),
        Err(PlatformError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
