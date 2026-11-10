//! Carrova manifest: OS-neutral transfer description, content-addressed chunks.
//!
//! A [`Manifest`] lists [`ManifestItem`]s, each made of [`Entry`]s addressed as
//! `(SemanticRoot, RelPath)`. File contents are split with FastCDC into
//! BLAKE3-addressed chunks ([`chunk_reader`]). The wire format is CBOR with a
//! leading version check ([`Manifest::to_cbor`] / [`Manifest::from_cbor`]).
//! Senders hand chunk bytes to the archive writer or the LAN server through
//! [`ChunkSource`].

mod chunk;
mod entry;
mod error;
mod hash;
mod model;
mod path;
mod source;
mod text;
mod validate;
mod wire;

pub use chunk::{
    ChunkParams, ChunkRef, DEFAULT_CHUNK_PARAMS, FileChunks, MAX_CHUNK_LEN, chunk_file,
    chunk_reader,
};
pub use entry::{entry_from_path, time_from_unix_nanos, unix_nanos};
pub use error::{ChunkFault, InvalidReason, ManifestError, Result};
pub use hash::{Blake3Hash, ChunkHash};
pub use model::{
    Artifact, Entry, EntryKind, FORMAT_VERSION, ItemFlags, MAX_ARTIFACTS_PER_ITEM, MAX_ENTRIES,
    MAX_ITEMS, Manifest, ManifestItem, Portability, SourceInfo,
};
pub use path::RelPath;
pub use source::{ChunkSource, ChunkSourceError};
pub use text::is_unsafe_char;
pub use wire::{DEFAULT_MAX_MANIFEST_BYTES, SUPPORTED_FORMAT};
