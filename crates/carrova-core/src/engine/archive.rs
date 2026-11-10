//! The archive an import reads from, reopened (and re-checked) for
//! staging and resume.

use std::path::{Path, PathBuf};

use carrova_archive::ArchiveReader;
use carrova_crypto::Passphrase;
use carrova_manifest::{Blake3Hash, Manifest};
use carrova_platform::Platform;

use crate::error::{CoreError, Result};
use crate::store::ArchiveFingerprint;

/// An import's archive file and its passphrase (in memory only, zeroized
/// on drop; never persisted or logged).
#[derive(Debug)]
pub(super) struct ArchiveSource {
    path: PathBuf,
    pass: Passphrase,
}

impl ArchiveSource {
    /// The archive at `path` (absolute, as recorded), opened with a copy of
    /// `pass`.
    pub(super) fn new(path: &Path, pass: &Passphrase) -> Self {
        Self {
            path: path.to_path_buf(),
            pass: Passphrase::new(pass.expose().to_owned()),
        }
    }

    /// Reopen the archive and check it is the one `expected` was taken
    /// from (when the import was opened); otherwise
    /// [`CoreError::Verification`] and nothing is read from it.
    pub(super) fn reopen(
        &self,
        platform: &dyn Platform,
        expected: Option<&ArchiveFingerprint>,
    ) -> Result<ArchiveReader> {
        let reader = ArchiveReader::open(platform, &self.path, &self.pass)?;
        if expected != Some(&fingerprint(&reader)?) {
            return Err(CoreError::Verification("archive changed since plan".into()));
        }
        Ok(reader)
    }

    /// The manifest of the reopened (and re-checked) archive.
    pub(super) fn manifest(
        &self,
        platform: &dyn Platform,
        expected: Option<&ArchiveFingerprint>,
    ) -> Result<Manifest> {
        Ok(self.reopen(platform, expected)?.manifest().clone())
    }
}

/// Digests identifying an opened archive (see [`ArchiveFingerprint`]).
pub(super) fn fingerprint(reader: &ArchiveReader) -> Result<ArchiveFingerprint> {
    Ok(ArchiveFingerprint {
        header: Blake3Hash::of(&reader.header().to_bytes()),
        manifest: Blake3Hash::of(&reader.manifest().to_cbor_vec()?),
    })
}
