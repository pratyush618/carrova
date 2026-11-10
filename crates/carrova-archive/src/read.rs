//! Reading an archive: [`ArchiveReader`] authenticates, decodes and
//! verifies records as they stream.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use carrova_crypto::{DecryptReader, Passphrase};
use carrova_manifest::{
    Blake3Hash, ChunkHash, DEFAULT_MAX_MANIFEST_BYTES, Manifest, ManifestError,
};
use carrova_platform::Platform;

use crate::error::{ArchiveError, Result, body_err, malformed};
use crate::header::{
    ArchiveHeader, HEADER_LEN, MAGIC, MAX_CHUNK_LEN, TAG_CHUNK, TAG_END, TAG_MANIFEST,
};

/// Expected length per chunk hash. [`Manifest::validate`] (run by both
/// encoding and decoding) already bounds every length to
/// `1..=`[`MAX_CHUNK_LEN`] and gives each hash a single length.
fn chunk_lens(manifest: &Manifest) -> HashMap<ChunkHash, u32> {
    manifest
        .unique_chunk_refs()
        .map(|c| (c.hash, c.len))
        .collect()
}

/// Streaming reader. [`ArchiveReader::open`] derives the key, decrypts and
/// validates the manifest; [`ArchiveReader::next_chunk`] yields verified
/// chunks until the END record, then checks completeness.
///
/// Chunks are authenticated and hash-verified individually as they are
/// returned; the archive as a whole is only proven complete (no missing
/// chunks, not truncated) when `next_chunk` returns `Ok(None)`. Callers
/// must not act on staged chunks before that.
#[derive(Debug)]
pub struct ArchiveReader {
    header: ArchiveHeader,
    manifest: Manifest,
    body: DecryptReader<File>,
    /// Chunks still expected, with their lengths.
    pending: HashMap<ChunkHash, u32>,
    received: HashSet<ChunkHash>,
    count: u64,
    state: ReaderState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderState {
    Chunks,
    Done,
    Failed,
}

impl ArchiveReader {
    /// Open and authenticate an archive. Wrong passphrase → `Crypto(Decrypt)`.
    pub fn open(platform: &dyn Platform, path: &Path, pass: &Passphrase) -> Result<Self> {
        let mut file = platform.open_read_nofollow(path)?;
        let mut raw = [0u8; HEADER_LEN];
        let got = read_up_to(&mut file, &mut raw)?;
        if got < MAGIC.len() || raw[..MAGIC.len()] != MAGIC {
            return Err(ArchiveError::BadMagic);
        }
        if got < HEADER_LEN {
            return Err(malformed("truncated header"));
        }
        let header = ArchiveHeader::from_bytes(&raw)?;
        let key = carrova_crypto::derive_key(pass, &header.salt, header.kdf)?;
        let mut body = DecryptReader::new(file, &key, &header.nonce, &raw)?;
        drop(key);

        let tag = read_u8(&mut body)?;
        if tag != TAG_MANIFEST {
            return Err(malformed("first record is not the manifest"));
        }
        let len = read_u64(&mut body)?;
        if len > DEFAULT_MAX_MANIFEST_BYTES {
            return Err(ManifestError::TooLarge {
                limit: DEFAULT_MAX_MANIFEST_BYTES,
            }
            .into());
        }
        // Consumes exactly the `len`-byte record.
        let record = (&mut body).take(len);
        let manifest = Manifest::from_cbor_limited(record, len).map_err(|e| match e {
            ManifestError::Io(io) => body_err(io),
            ManifestError::TrailingBytes => malformed("trailing bytes in manifest record"),
            other => other.into(),
        })?;
        let pending = chunk_lens(&manifest);
        tracing::debug!(chunks = pending.len(), "archive manifest authenticated");
        Ok(Self {
            header,
            manifest,
            body,
            pending,
            received: HashSet::new(),
            count: 0,
            state: ReaderState::Chunks,
        })
    }

    /// Plaintext header.
    #[must_use]
    pub fn header(&self) -> &ArchiveHeader {
        &self.header
    }

    /// Validated manifest.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Next verified chunk, or `None` after a complete END record. After an
    /// error the reader is poisoned and keeps returning errors.
    pub fn next_chunk(&mut self) -> Result<Option<(ChunkHash, Vec<u8>)>> {
        match self.state {
            ReaderState::Done => return Ok(None),
            ReaderState::Failed => return Err(malformed("archive reader failed earlier")),
            ReaderState::Chunks => {}
        }
        let r = self.read_record();
        match &r {
            Err(_) => self.state = ReaderState::Failed,
            Ok(None) => self.state = ReaderState::Done,
            Ok(Some(_)) => {}
        }
        r
    }

    /// Iterator over [`Self::next_chunk`]; stops after the first error.
    pub fn chunks(&mut self) -> impl Iterator<Item = Result<(ChunkHash, Vec<u8>)>> + '_ {
        let mut stop = false;
        std::iter::from_fn(move || {
            if stop {
                return None;
            }
            let item = self.next_chunk().transpose();
            stop = matches!(item, None | Some(Err(_)));
            item
        })
    }

    fn read_record(&mut self) -> Result<Option<(ChunkHash, Vec<u8>)>> {
        match read_u8(&mut self.body)? {
            TAG_CHUNK => {
                let mut hash = [0u8; 32];
                self.body.read_exact(&mut hash).map_err(body_err)?;
                let hash = Blake3Hash(hash);
                let len = read_u32(&mut self.body)?;
                if len == 0 || len > MAX_CHUNK_LEN {
                    return Err(malformed(format!("chunk {hash} has invalid length {len}")));
                }
                // Decide before allocating/reading the payload.
                match self.pending.remove(&hash) {
                    Some(expected) if expected == len => {}
                    Some(_) => {
                        return Err(malformed(format!(
                            "chunk {hash} length differs from manifest"
                        )));
                    }
                    None if self.received.contains(&hash) => {
                        return Err(malformed(format!("duplicate chunk {hash}")));
                    }
                    None => {
                        return Err(malformed(format!(
                            "chunk {hash} is not referenced by the manifest"
                        )));
                    }
                }
                let mut bytes = vec![0u8; len as usize];
                self.body.read_exact(&mut bytes).map_err(body_err)?;
                if Blake3Hash::of(&bytes) != hash {
                    return Err(ArchiveError::ChunkHashMismatch(hash));
                }
                self.received.insert(hash);
                self.count += 1;
                Ok(Some((hash, bytes)))
            }
            TAG_END => {
                let count = read_u64(&mut self.body)?;
                if count != self.count {
                    return Err(malformed(format!(
                        "END record counts {count} chunks, read {}",
                        self.count
                    )));
                }
                if !self.pending.is_empty() {
                    return Err(ArchiveError::MissingChunks(self.pending.len()));
                }
                // The body must end here (this also completes STREAM
                // authentication of the final segment).
                let mut extra = [0u8; 1];
                if read_up_to(&mut self.body, &mut extra)? != 0 {
                    return Err(malformed("data after END record"));
                }
                tracing::debug!(chunks = self.count, "archive fully verified");
                Ok(None)
            }
            TAG_MANIFEST => Err(malformed("second manifest record")),
            other => Err(malformed(format!("unknown record tag {other}"))),
        }
    }
}

/// Read until `buf` is full or EOF; returns bytes read.
fn read_up_to(r: &mut impl Read, buf: &mut [u8]) -> Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(n)
}

fn read_u8(r: &mut impl Read) -> Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b).map_err(body_err)?;
    Ok(b[0])
}

fn read_u32(r: &mut impl Read) -> Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).map_err(body_err)?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64(r: &mut impl Read) -> Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).map_err(body_err)?;
    Ok(u64::from_le_bytes(b))
}
