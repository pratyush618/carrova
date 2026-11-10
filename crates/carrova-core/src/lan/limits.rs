//! LAN session limits: timeouts, batch sizes, pairing attempts and the
//! receiver's disk guard.

use std::path::Path;
use std::time::Duration;

use carrova_manifest::ChunkRef;
use carrova_platform::Platform;

use crate::error::{CoreError, Result};
use crate::stage::staging::{CHECKPOINT_BYTES, LOG_BYTES_PER_CHUNK};

/// Largest manifest part in one [`super::LanMsg::Manifest`] (well below the
/// transport's frame limit, leaving room for the CBOR envelope).
pub const MANIFEST_PART_LEN: usize = 512 * 1024;
/// Chunk bytes the receiver pulls per [`super::LanMsg::Fetch`]; the disk guard
/// ([`check_space`]) runs and a checkpoint is taken between batches. A
/// whole multiple of the staging checkpoint size (16 MiB), so the periodic
/// checkpoints inside a batch line up with its end and a killed receiver
/// re-fetches at most one checkpoint's worth.
pub const TRANSFER_BATCH_BYTES: u64 = 4 * CHECKPOINT_BYTES;
/// Sender: failed pairing attempts (wrong codes) one send session
/// tolerates. Each failure burns the code; after one the user is warned and
/// asked before a new code is offered; this one ends the session
/// ([`CoreError::PairingAttemptsExhausted`]). Bounds online guessing of the
/// ~20.7-bit code to this many tries.
pub const MAX_PAIRING_FAILURES: u32 = 3;
/// Receiver: how long pairing (discovery + handshake) may take.
pub const PAIR_TIMEOUT: Duration = Duration::from_secs(60);
/// How long one side waits for a control message the other side sends
/// right away (the manifest after the inventory).
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(60);
/// Sender: how long each wait for a pairing receiver or the receiver's
/// next message lasts before it is simply renewed (the user may take
/// their time; a dead peer shows up as a lost connection instead).
pub const IDLE_POLL: Duration = Duration::from_secs(60);
/// How long either side keeps trying to reconnect after the connection
/// dropped before giving up (the session stays resumable).
pub const RECONNECT_WINDOW: Duration = Duration::from_secs(10 * 60);
/// Receiver: first pause between reconnect attempts (doubles each time)…
pub const RECONNECT_BACKOFF_MIN: Duration = Duration::from_secs(1);
/// …up to this.
pub const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(10);
/// Receiver: how long one reconnect attempt (discovery + resume proof) may
/// take.
pub const RECONNECT_ATTEMPT: Duration = Duration::from_secs(10);
/// Receiver: how long it tries to deliver the final notice (checklist or
/// abort) after the session ended; best effort.
pub const FINAL_NOTICE_WINDOW: Duration = Duration::from_secs(30);

/// Free space the staging filesystem must keep during a transfer (see
/// [`check_space`]): `progress.log`, the session record and the apply
/// journal grow there even though staged files are pre-sized.
pub const MIN_FREE_DURING_TRANSFER: u64 = 16 * 1024 * 1024;
/// How often a transfer paused for disk space re-checks [`check_space`].
pub const SPACE_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Disk guard the receiver runs before fetching each batch `upcoming`:
/// the filesystem of `staging_dir` must keep [`MIN_FREE_DURING_TRANSFER`]
/// plus the batch's `progress.log` growth free. Staged files are
/// preallocated when the staging store opens, so chunk bytes need no
/// extra room (on a filesystem that cannot preallocate, a full disk
/// shows up as [`CoreError::DiskFull`] from the store's `put` instead).
///
/// On [`CoreError::DiskFull`] the transfer emits
/// `TransferPaused { LowDiskSpace }`, keeps the connection, re-checks every
/// [`SPACE_POLL_INTERVAL`] and emits `TransferResumed` once this passes;
/// the same applies to `DiskFull` from opening the store or from `put`.
pub fn check_space(
    platform: &dyn Platform,
    staging_dir: &Path,
    upcoming: &[ChunkRef],
) -> Result<()> {
    let records = (upcoming.len() as u64).saturating_mul(LOG_BYTES_PER_CHUNK);
    let needed = MIN_FREE_DURING_TRANSFER.saturating_add(records);
    let available = platform.free_space(staging_dir)?;
    if available < needed {
        return Err(CoreError::DiskFull { needed, available });
    }
    Ok(())
}
