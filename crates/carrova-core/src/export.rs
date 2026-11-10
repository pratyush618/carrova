//! Export (sender side): write a [`Capture`] to an encrypted archive file.

use std::path::Path;

use carrova_archive::{ArchiveError, ArchiveSummary, WriteOptions};
use carrova_crypto::Passphrase;
use carrova_manifest::ChunkSourceError;
use carrova_platform::Platform;

use crate::cancel::{Cancel, Cancellable};
use crate::capture::Capture;
use crate::error::{CoreError, Result};
use crate::session::{EventSink, Metered, Phase, ProgressMeter};

/// Write `capture` to `path` (fails if it exists; file mode 600).
///
/// Emits `Progress { phase: Archive }` per chunk (bytes of unique chunks).
/// A file that changed since capture makes the write fail, and `cancel`
/// stops it before the next chunk ([`CoreError::Cancelled`]); no partial
/// archive is left behind either way.
pub fn write(
    platform: &dyn Platform,
    capture: &Capture,
    path: &Path,
    pass: &Passphrase,
    opts: WriteOptions,
    sink: &dyn EventSink,
    cancel: &Cancel,
) -> Result<ArchiveSummary> {
    let total = capture
        .manifest
        .unique_chunk_refs()
        .map(|c| u64::from(c.len))
        .fold(0, u64::saturating_add);
    let mut meter =
        ProgressMeter::new(sink, Phase::Archive, total, ProgressMeter::EVERY_UPDATE).start();
    let source = Cancellable::new(capture.chunk_source(platform), cancel);
    let mut source = Metered::new(source, &mut meter);
    carrova_archive::write_archive(platform, path, pass, &capture.manifest, &mut source, opts)
        .map_err(|e| match e {
            ArchiveError::Source(ChunkSourceError::Cancelled) => CoreError::Cancelled,
            e => e.into(),
        })
}
