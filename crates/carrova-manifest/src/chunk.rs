use std::fs::File;
use std::io::Read;
use std::path::Path;

use fastcdc::v2020::{self, StreamCDC};
use serde::{Deserialize, Serialize};

use crate::{Blake3Hash, ChunkHash, InvalidReason, ManifestError, Result};

/// Content-defined chunking parameters, in bytes.
///
/// Fields are public: a plain parameter record checked by
/// [`ChunkParams::validate`] wherever it is used, with no invariant to hide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkParams {
    pub min: u32,
    pub avg: u32,
    pub max: u32,
}

/// Carrova's default: 16 KiB / 64 KiB / 256 KiB.
pub const DEFAULT_CHUNK_PARAMS: ChunkParams = ChunkParams {
    min: 16 * 1024,
    avg: 64 * 1024,
    max: 256 * 1024,
};

/// Longest chunk any manifest may list: the default chunker maximum. One
/// source of truth for every reader of untrusted manifests (archive, LAN),
/// enforced by [`crate::Manifest::validate`], so a peer cannot make a
/// receiver allocate more than this per chunk.
pub const MAX_CHUNK_LEN: u32 = DEFAULT_CHUNK_PARAMS.max;

impl Default for ChunkParams {
    fn default() -> Self {
        DEFAULT_CHUNK_PARAMS
    }
}

impl ChunkParams {
    /// Check the parameters against FastCDC's supported ranges
    /// (the chunker would otherwise panic).
    pub fn validate(&self) -> Result<()> {
        let bad = |m| Err(ManifestError::InvalidChunkParams(m));
        if !(v2020::MINIMUM_MIN..=v2020::MINIMUM_MAX).contains(&self.min) {
            return bad("min out of range (64 B ..= 1 MiB)");
        }
        if !(v2020::AVERAGE_MIN..=v2020::AVERAGE_MAX).contains(&self.avg) {
            return bad("avg out of range (256 B ..= 4 MiB)");
        }
        if !(v2020::MAXIMUM_MIN..=v2020::MAXIMUM_MAX).contains(&self.max) {
            return bad("max out of range (1 KiB ..= 16 MiB)");
        }
        if !(self.min <= self.avg && self.avg <= self.max) {
            return bad("require min <= avg <= max");
        }
        Ok(())
    }
}

/// Reference to one chunk of a file.
///
/// A wire DTO: fields are public because it is (de)serialised as part of
/// the manifest and built by the chunker; consistency (lengths, contiguity,
/// one length per hash) is enforced on the whole manifest by
/// [`crate::Manifest::validate`], not per value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChunkRef {
    /// BLAKE3 of the chunk bytes.
    pub hash: ChunkHash,
    /// Byte offset of the chunk within its file.
    pub offset: u64,
    /// Chunk length in bytes (never zero).
    pub len: u32,
}

/// Result of chunking one byte stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChunks {
    /// BLAKE3 of the whole stream.
    pub file_hash: Blake3Hash,
    /// Total bytes read.
    pub size: u64,
    /// Chunks in order; contiguous, covering `0..size`. Empty for empty input.
    pub chunks: Vec<ChunkRef>,
}

/// Chunk a stream with FastCDC, hashing each chunk and the whole stream with
/// BLAKE3. Memory use is bounded by `params.max`, independent of input size.
///
/// Inputs no larger than `params.min` yield exactly one chunk; empty input
/// yields none.
pub fn chunk_reader(reader: impl Read, params: ChunkParams) -> Result<FileChunks> {
    params.validate()?;
    let mut whole = blake3::Hasher::new();
    let mut chunks = Vec::new();
    let mut size: u64 = 0;
    for item in StreamCDC::new(reader, params.min, params.avg, params.max) {
        let data = item.map_err(std::io::Error::from)?;
        whole.update(&data.data);
        let len = u32::try_from(data.length)
            .map_err(|_| ManifestError::Invalid(InvalidReason::ChunkTooLong))?;
        chunks.push(ChunkRef {
            hash: Blake3Hash::of(&data.data),
            offset: data.offset,
            len,
        });
        size = size.saturating_add(u64::from(len));
    }
    Ok(FileChunks {
        file_hash: whole.finalize().into(),
        size,
        chunks,
    })
}

/// Open and chunk the file at `path`. **Follows symlinks**: receiver/capture
/// code must use `Platform::open_read_nofollow` + [`chunk_reader`] instead
/// (as [`crate::entry_from_path`] does).
pub fn chunk_file(path: &Path, params: ChunkParams) -> Result<FileChunks> {
    let file = File::open(path).map_err(|source| ManifestError::IoAt {
        path: path.to_path_buf(),
        source,
    })?;
    chunk_reader(std::io::BufReader::new(file), params).map_err(|e| match e {
        ManifestError::Io(source) => ManifestError::IoAt {
            path: path.to_path_buf(),
            source,
        },
        other => other,
    })
}
