//! Free-space policy (receiver): how much room a plan needs on each
//! filesystem it writes to (`need`), which filesystem that is
//! (`location`) and the per-filesystem tally (`account`).
//!
//! Staged files live in Carrova's state dir and are renamed into place by
//! apply, so when the state dir and a target root share a filesystem the
//! data occupies it once; across filesystems the staged copy (state-dir
//! filesystem) and the final copy (target filesystem) both need room.

use carrova_manifest::Manifest;
use carrova_platform::Platform;

use std::path::Path;

use crate::error::{CoreError, Result};
use crate::plan::Plan;
use crate::stage::StageLayout;

mod account;
mod location;
mod need;

use account::SpaceAccount;
pub(crate) use account::nearest_existing;
pub use location::{FsLocation, SpaceShortfall};
pub use need::SpaceNeed;

/// Fixed part of the per-filesystem margin (see [`required_space`]).
pub const SPACE_MARGIN_FIXED: u64 = 64 * 1024 * 1024;
/// Proportional part of the margin: `bytes / SPACE_MARGIN_DIVISOR` (5 %).
pub const SPACE_MARGIN_DIVISOR: u64 = 20;

/// Free space one filesystem needs to take `bytes` of payload: the bytes
/// plus 5 % for filesystem overhead and a fixed [`SPACE_MARGIN_FIXED`] for
/// the journal, backups of small files and slack:
/// `bytes + bytes / 20 + 64 MiB`.
#[must_use]
pub fn required_space(bytes: u64) -> u64 {
    bytes
        .saturating_add(bytes / SPACE_MARGIN_DIVISOR)
        .saturating_add(SPACE_MARGIN_FIXED)
}

/// Free space `plan` needs on each filesystem it writes to (state-dir
/// filesystem first), for display. The planner applies the same policy
/// and turns every short filesystem into a blocker. A plan that does not
/// match `manifest` is a [`crate::CoreError::Verification`] error.
pub fn space_needs(
    platform: &dyn Platform,
    plan: &Plan,
    manifest: &Manifest,
) -> Result<Vec<SpaceNeed>> {
    Ok(needs(platform, &StageLayout::new(plan, manifest)?))
}

/// Needs of everything `layout` stages: each staged file occupies the
/// state-dir filesystem and, unless that is also its target root's
/// filesystem, the target filesystem too. Artifacts (no target root) are
/// counted on the state-dir filesystem only.
pub(crate) fn needs(platform: &dyn Platform, layout: &StageLayout) -> Vec<SpaceNeed> {
    let mut acc = SpaceAccount::new(platform);
    for f in layout.files() {
        acc.add(f.root, f.size);
    }
    acc.finish()
}

/// `StorageFull` → [`CoreError::DiskFull`] (`needed` bytes, free space of
/// `dir`); anything else unchanged.
pub(crate) fn disk_full_or(
    platform: &dyn Platform,
    dir: &Path,
    e: std::io::Error,
    needed: u64,
) -> CoreError {
    if e.kind() != std::io::ErrorKind::StorageFull {
        return e.into();
    }
    let available = platform.free_space(dir).unwrap_or(0);
    CoreError::DiskFull { needed, available }
}
