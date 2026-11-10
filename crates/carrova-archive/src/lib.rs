//! Carrova archive: single-file encrypted export/import.
//!
//! Layout:
//! ```text
//! header (plaintext, authenticated as AAD of every segment):
//!   magic "CARROVA\0" | u16 version | u16 flags (0; compression reserved)
//!   | u32 m_kib | u32 t | u32 p | salt[16] | nonce_prefix[19]
//! body (carrova-crypto STREAM):
//!   record MANIFEST: u8 tag=1 | u64 len | manifest CBOR (carrova-manifest v2)
//!   record CHUNK:    u8 tag=2 | hash[32] | u32 len | bytes   (unique_chunk_refs() order)
//!   record END:      u8 tag=0 | u64 chunk_count
//! ```
//! All integers are little-endian. The header is [`HEADER_LEN`] bytes; its
//! exact serialized bytes are the AAD of every body segment, so changing any
//! header field (including KDF parameters, salt and nonce) fails
//! decryption. The body ends exactly after the END record: the STREAM's
//! last-segment flag rejects truncation and appended bytes.
//!
//! The reader verifies every chunk's BLAKE3 hash, rejects unknown/duplicate
//! chunks, and checks that every chunk the manifest references arrived.
//! Uses the same manifest format as network mode.
//!
//! The archive file itself is opened with
//! [`Platform::open_read_nofollow`](carrova_platform::Platform::open_read_nofollow)
//! (a symlink at the user-chosen path is refused) and written through
//! [`Platform::create_new_file`](carrova_platform::Platform::create_new_file) +
//! [`Platform::rename_noreplace`](carrova_platform::Platform::rename_noreplace).

mod error;
mod header;
mod read;
mod write;

pub use error::{ArchiveError, Result};
pub use header::{ARCHIVE_VERSION, ArchiveHeader, HEADER_LEN, MAGIC, MAX_CHUNK_LEN};
pub use read::ArchiveReader;
pub use write::{ArchiveSummary, WriteOptions, write_archive};

#[cfg(test)]
#[path = "../tests/lib/unit.rs"]
mod tests;
