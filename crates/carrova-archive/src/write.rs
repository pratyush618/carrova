//! Writing an archive: temp file, header, encrypted records, rename.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use carrova_crypto::{ArchiveKey, EncryptWriter, KdfParams, NoncePrefix, Passphrase, Salt};
use carrova_manifest::{Blake3Hash, ChunkSource, DEFAULT_MAX_MANIFEST_BYTES, Manifest};
use carrova_platform::Platform;
use serde::{Deserialize, Serialize};

use crate::error::{ArchiveError, Result, malformed};
use crate::header::{ARCHIVE_VERSION, ArchiveHeader, TAG_CHUNK, TAG_END, TAG_MANIFEST};

/// What was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArchiveSummary {
    /// Distinct chunks stored.
    pub chunks: u64,
    /// Archive file size in bytes.
    pub bytes: u64,
}

/// Options for writing.
#[derive(Debug, Clone, Copy)]
pub struct WriteOptions {
    /// Must pass `KdfParams::validate_for_write`.
    pub kdf: KdfParams,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            kdf: KdfParams::DEFAULT,
        }
    }
}

/// Write `manifest` and all its unique chunks to `path`. Writes a sibling
/// temp file (mode `600`, create-new via `platform`) and renames it into
/// place only after a successful finish; fails if `path` exists. The temp
/// file is removed on any failure.
pub fn write_archive(
    platform: &dyn Platform,
    path: &Path,
    pass: &Passphrase,
    manifest: &Manifest,
    source: &mut dyn ChunkSource,
    opts: WriteOptions,
) -> Result<ArchiveSummary> {
    opts.kdf.validate_for_write()?;
    write_with_kdf(platform, path, pass, manifest, source, opts.kdf)
}

/// [`write_archive`] without the writer floor (tests use cheap KDF params;
/// the read-side bounds are still enforced by `derive_key`).
pub(crate) fn write_with_kdf(
    platform: &dyn Platform,
    path: &Path,
    pass: &Passphrase,
    manifest: &Manifest,
    source: &mut dyn ChunkSource,
    kdf: KdfParams,
) -> Result<ArchiveSummary> {
    let (dir, tmp) = staging_paths(platform, path)?;

    // Encode before touching the filesystem: an invalid manifest never
    // produces a file.
    let cbor = manifest.to_cbor_vec()?;
    if cbor.len() as u64 > DEFAULT_MAX_MANIFEST_BYTES {
        return Err(malformed("manifest exceeds the archive size limit"));
    }

    let header = ArchiveHeader {
        version: ARCHIVE_VERSION,
        flags: 0,
        kdf,
        salt: Salt::random()?,
        nonce: NoncePrefix::random()?,
    };
    let key = carrova_crypto::derive_key(pass, &header.salt, kdf)?;

    let file = platform.create_new_file(&tmp, Some(0o600))?;
    let written = write_body(file, &header, &key, &cbor, manifest, source);
    publish(platform, written, &tmp, path, &dir)
}

/// The directory holding `path` and a fresh hidden temp path beside it;
/// fails if `path` already exists.
fn staging_paths(platform: &dyn Platform, path: &Path) -> Result<(PathBuf, PathBuf)> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "archive path has no file name")
    })?;
    if platform.metadata(path).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        )
        .into());
    }
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let tmp = dir.join(temp_name(name));
    Ok((dir, tmp))
}

/// Rename the finished temp file to `path` (then sync `dir`), or remove it
/// if writing or renaming failed.
fn publish(
    platform: &dyn Platform,
    written: Result<(u64, u64)>,
    tmp: &Path,
    path: &Path,
    dir: &Path,
) -> Result<ArchiveSummary> {
    let result = written.and_then(|(chunks, bytes)| {
        platform.rename_noreplace(tmp, path)?;
        Ok(ArchiveSummary { chunks, bytes })
    });
    match result {
        Ok(summary) => {
            if let Err(e) = platform.sync_dir(dir) {
                tracing::warn!(dir = %dir.display(), error = %e, "archive written but directory sync failed");
            }
            tracing::debug!(
                path = %path.display(),
                chunks = summary.chunks,
                bytes = summary.bytes,
                "archive written"
            );
            Ok(summary)
        }
        Err(e) => {
            if let Err(rm) = platform.remove_file(tmp) {
                tracing::warn!(tmp = %tmp.display(), error = %rm, "failed to remove temp archive");
            }
            Err(e)
        }
    }
}

/// Header, then the encrypted manifest record, every unique chunk (each
/// re-verified against the manifest) and the end record; fsynced.
/// Returns `(chunks, file bytes)`.
fn write_body(
    mut file: File,
    header: &ArchiveHeader,
    key: &ArchiveKey,
    cbor: &[u8],
    manifest: &Manifest,
    source: &mut dyn ChunkSource,
) -> Result<(u64, u64)> {
    let header_bytes = header.to_bytes();
    file.write_all(&header_bytes)?;
    let mut w = EncryptWriter::new(file, key, &header.nonce, &header_bytes)?;
    w.write_all(&[TAG_MANIFEST])?;
    w.write_all(&(cbor.len() as u64).to_le_bytes())?;
    w.write_all(cbor)?;
    let mut count: u64 = 0;
    for chunk in manifest.unique_chunk_refs() {
        let bytes = source.read_chunk(chunk)?;
        if bytes.len() != chunk.len as usize || Blake3Hash::of(&bytes) != chunk.hash {
            return Err(ArchiveError::ChunkHashMismatch(chunk.hash));
        }
        w.write_all(&[TAG_CHUNK])?;
        w.write_all(chunk.hash.as_bytes())?;
        w.write_all(&chunk.len.to_le_bytes())?;
        w.write_all(&bytes)?;
        count += 1;
    }
    w.write_all(&[TAG_END])?;
    w.write_all(&count.to_le_bytes())?;
    let file = w.finish()?;
    file.sync_all()?;
    Ok((count, file.metadata()?.len()))
}

/// Hidden sibling temp name, unique per process and call.
fn temp_name(name: &std::ffi::OsStr) -> std::ffi::OsString {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut s = std::ffi::OsString::from(".");
    s.push(name);
    s.push(format!(".carrova-tmp-{}-{n}", std::process::id()));
    s
}
