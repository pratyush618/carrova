use std::ops::RangeInclusive;
use std::path::PathBuf;

use carrova_platform::{PlatformError, SemanticRoot};

use crate::{ChunkHash, MAX_CHUNK_LEN, RelPath};

/// Errors produced by the manifest crate.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    /// I/O failure while reading a stream (no path known).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// I/O failure on a specific path.
    #[error("I/O error on {path}: {source}")]
    IoAt {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Error from the platform layer.
    #[error(transparent)]
    Platform(#[from] PlatformError),

    /// Chunker parameters outside the supported range.
    #[error("invalid chunk parameters: {0}")]
    InvalidChunkParams(&'static str),

    /// A relative path failed hygiene checks. `path` is the offending path
    /// in escaped form (`str::escape_debug`), safe to print on a terminal.
    #[error("invalid relative path \"{path}\": {reason}")]
    InvalidRelPath { path: String, reason: &'static str },

    /// A text field holds a control character (C0, DEL, C1) or a Unicode
    /// bidi control, which could spoof what a terminal shows. `escaped` is
    /// the value in escaped form (`str::escape_debug`); the raw string is
    /// never echoed.
    #[error("{field} contains a control or bidi character: \"{escaped}\"")]
    UnsafeText {
        field: &'static str,
        escaped: String,
    },

    /// A manifest exceeds a structural count limit (items, entries,
    /// artifacts per item).
    #[error("manifest has too many {what}: {count} (limit {limit})")]
    TooMany {
        what: &'static str,
        count: usize,
        limit: usize,
    },

    /// Bytes follow the encoded manifest in its input.
    #[error("trailing bytes after the encoded manifest")]
    TrailingBytes,

    /// A filesystem path (or symlink target) is not valid UTF-8.
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(PathBuf),

    /// The filesystem object is neither file, directory nor symlink.
    #[error("unsupported file type at {0}")]
    UnsupportedFileKind(PathBuf),

    /// A hex string could not be parsed as a 32-byte hash.
    #[error("invalid BLAKE3 hash hex string")]
    InvalidHashHex,

    /// The manifest carries a format version this build cannot read.
    #[error("unsupported manifest format version {found} (supported: {}..={})", supported.start(), supported.end())]
    UnsupportedFormatVersion {
        found: u32,
        supported: RangeInclusive<u32>,
    },

    /// The encoded data has no readable `format_version` field.
    #[error("manifest has no valid format_version field")]
    MissingFormatVersion,

    /// The manifest decoded but is internally inconsistent.
    #[error("invalid manifest: {0}")]
    Invalid(#[from] InvalidReason),

    /// Two entries address the same `(root, rel)` path.
    #[error("duplicate entry {root}/{path}")]
    DuplicateEntry { root: SemanticRoot, path: String },

    /// An entry lies below another entry (same root) that is a symlink or a
    /// file, so applying it would write through that entry.
    #[error("entry {root}/{path} lies below non-directory entry {root}/{ancestor}")]
    NonDirAncestor {
        root: SemanticRoot,
        path: String,
        ancestor: String,
    },

    /// An entry's mode has bits outside `0o777` (setuid, setgid, sticky or
    /// beyond).
    #[error("entry {root}/{path} has unsafe mode {mode:#o} (only 0o777 bits allowed)")]
    UnsafeMode {
        root: SemanticRoot,
        path: String,
        mode: u32,
    },

    /// The encoded manifest exceeds the decode size limit.
    #[error("manifest exceeds size limit of {limit} bytes")]
    TooLarge { limit: u64 },

    /// CBOR encoding failed.
    #[error("CBOR encode error: {0}")]
    Encode(String),

    /// CBOR decoding failed.
    #[error("CBOR decode error: {0}")]
    Decode(String),
}

/// Why a manifest is internally inconsistent ([`ManifestError::Invalid`]).
/// Item ids and artifact names are quoted only after they passed the
/// control / bidi character check.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidReason {
    /// An item has an empty `item_id`.
    #[error("empty item_id")]
    EmptyItemId,

    /// An item has `recipe_id: Some("")`.
    #[error("item {item}: empty recipe_id")]
    EmptyRecipeId { item: String },

    /// An item has `handler: Some("")`.
    #[error("item {item}: empty handler")]
    EmptyHandler { item: String },

    /// An item carries artifacts but no handler to apply them.
    #[error("item {item}: artifacts without handler")]
    ArtifactsWithoutHandler { item: String },

    /// An artifact name is empty or repeated within its item.
    #[error("item {item}: empty or duplicate artifact name")]
    BadArtifactName { item: String },

    /// An artifact's chunk list does not cover it exactly.
    #[error("artifact {name}: {fault}")]
    ArtifactChunks { name: String, fault: ChunkFault },

    /// A file entry's chunk list does not cover it exactly.
    #[error("{root}/{path}: {fault}")]
    EntryChunks {
        root: SemanticRoot,
        path: RelPath,
        fault: ChunkFault,
    },

    /// A symlink entry has an empty target.
    #[error("{root}/{path}: empty symlink target")]
    EmptySymlinkTarget { root: SemanticRoot, path: RelPath },

    /// One chunk hash is listed with more than one length.
    #[error("chunk {hash} listed with different lengths")]
    ConflictingChunkLength { hash: ChunkHash },

    /// The chunker produced a chunk whose length does not fit a `u32`.
    #[error("chunk longer than u32::MAX")]
    ChunkTooLong,
}

/// How a chunk list fails to cover `0..size` with contiguous chunks of
/// `1..=`[`MAX_CHUNK_LEN`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChunkFault {
    /// A chunk is empty or longer than [`MAX_CHUNK_LEN`].
    #[error("chunk length {len} outside 1..={MAX_CHUNK_LEN}")]
    LengthOutOfRange { len: u32 },

    /// A chunk does not start where the previous one ended.
    #[error("chunks not contiguous")]
    NotContiguous,

    /// The chunk offsets overflow `u64`.
    #[error("chunk offsets overflow")]
    OffsetOverflow,

    /// The chunks end before or after the declared size.
    #[error("chunks do not cover file size")]
    SizeMismatch,
}

/// Crate result alias.
pub type Result<T, E = ManifestError> = std::result::Result<T, E>;
