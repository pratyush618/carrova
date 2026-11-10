//! A sender's capture persisted in its session dir, so a LAN send can be
//! served after a restart.

use std::path::Path;

use carrova_manifest::{Blake3Hash, Manifest};
use carrova_platform::Platform;
use serde::{Deserialize, Serialize};

use super::source::CaptureSource;
use super::{CAPTURE_FILE, Capture, MANIFEST_FILE};
use crate::checklist::Skipped;
use crate::error::{CoreError, Corruption, EncodeError, Result};
use crate::store::{read_file, write_atomic};

/// Format version of [`CAPTURE_FILE`].
const CAPTURE_VERSION: u16 = 1;

/// On-disk form of [`CAPTURE_FILE`].
#[derive(Deserialize)]
struct CaptureOnDisk {
    version: u16,
    /// BLAKE3 of the [`MANIFEST_FILE`] bytes this capture belongs to.
    manifest: Blake3Hash,
    failed: Vec<Skipped>,
    source: CaptureSource,
}

/// Borrowing twin of [`CaptureOnDisk`] for encoding.
#[derive(Serialize)]
struct CaptureOnDiskRef<'a> {
    version: u16,
    manifest: Blake3Hash,
    failed: &'a [Skipped],
    source: &'a CaptureSource,
}

impl Capture {
    /// Persist the capture into `session_dir` so a sender can serve it
    /// after a restart: [`MANIFEST_FILE`] and [`CAPTURE_FILE`] (chunk
    /// locations as semantic root + relative path, handler artifact bytes,
    /// failed items), each written atomically (`600`). Returns the
    /// manifest bytes written.
    pub fn persist(&self, platform: &dyn Platform, session_dir: &Path) -> Result<Vec<u8>> {
        let manifest = self.manifest_cbor()?;
        let mut bytes = Vec::new();
        ciborium::into_writer(
            &CaptureOnDiskRef {
                version: CAPTURE_VERSION,
                manifest: Blake3Hash::of(&manifest),
                failed: &self.failed,
                source: &self.source,
            },
            &mut bytes,
        )
        .map_err(|e| CoreError::Encode {
            what: CAPTURE_FILE.into(),
            source: EncodeError::from(e),
        })?;
        write_atomic(platform, session_dir, CAPTURE_FILE, &bytes)?;
        write_atomic(platform, session_dir, MANIFEST_FILE, &manifest)?;
        Ok(manifest)
    }

    /// Load a capture persisted by [`Self::persist`]. The manifest is
    /// validated and must be the one the locations were recorded for;
    /// chunks are still re-read and re-verified on every
    /// [`Self::chunk_source`] read.
    pub fn load(platform: &dyn Platform, session_dir: &Path) -> Result<Self> {
        let read = |name: &'static str| {
            read_file(platform, &session_dir.join(name))?.ok_or(CoreError::MissingFile(name))
        };
        let corrupt = |reason: Corruption| CoreError::CorruptFile {
            file: CAPTURE_FILE.into(),
            reason,
        };
        let manifest_bytes = read(MANIFEST_FILE)?;
        let capture_bytes = read(CAPTURE_FILE)?;
        let on_disk: CaptureOnDisk = ciborium::from_reader(capture_bytes.as_slice())
            .map_err(|e| corrupt(Corruption::from(e)))?;
        if on_disk.version != CAPTURE_VERSION {
            return Err(corrupt(Corruption::Version(on_disk.version)));
        }
        if on_disk.manifest != Blake3Hash::of(&manifest_bytes) {
            return Err(corrupt(Corruption::Mismatch(MANIFEST_FILE)));
        }
        if !on_disk.source.is_consistent() {
            return Err(corrupt(Corruption::Dangling));
        }
        let manifest = Manifest::from_cbor(manifest_bytes.as_slice())?;
        manifest.validate()?;
        Ok(Self {
            manifest,
            failed: on_disk.failed,
            source: on_disk.source,
        })
    }
}
