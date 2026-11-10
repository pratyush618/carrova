use std::io::{Read, Write};
use std::ops::RangeInclusive;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{Manifest, ManifestError, Result};

/// Manifest format versions this build can decode.
pub const SUPPORTED_FORMAT: RangeInclusive<u32> = 1..=3;

/// Default cap on encoded manifest size accepted by [`Manifest::from_cbor`]:
/// 32 MiB.
///
/// A manifest is metadata only. Measured CBOR cost: about 65 bytes per
/// chunk reference and about 200 bytes per small single-chunk file entry
/// (field names, path, two hashes). 32 MiB therefore holds roughly 500k
/// chunks (over 30 GiB of file data at the 64 KiB average chunk size) or
/// about 150k small files: configs, keys and app data of a developer home
/// with room to spare. 16 MiB (~75k small files) was judged too tight for
/// homes with a large `documents` tree. Decoding is typed (no intermediate
/// `Value` tree), so memory stays within a small multiple of this bound.
pub const DEFAULT_MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;

/// Maximum CBOR nesting depth accepted on decode. A valid manifest nests
/// about 10 levels deep; ciborium's default (256) is looser than needed.
const MAX_NESTING: usize = 64;

/// Only the version field; every other field is ignored (no
/// `deny_unknown_fields`), so a newer format's body never causes a schema
/// error before the version is checked.
#[derive(Deserialize)]
struct Peek {
    format_version: u32,
}

pub(crate) fn check_version(found: u32) -> Result<()> {
    if SUPPORTED_FORMAT.contains(&found) {
        Ok(())
    } else {
        Err(ManifestError::UnsupportedFormatVersion {
            found,
            supported: SUPPORTED_FORMAT,
        })
    }
}

impl Manifest {
    /// Encode as CBOR. Validates first, so an invalid manifest is never emitted.
    pub fn to_cbor(&self, writer: impl Write) -> Result<()> {
        self.validate()?;
        ciborium::into_writer(self, writer).map_err(|e| match e {
            ciborium::ser::Error::Io(io) => ManifestError::Io(io),
            ciborium::ser::Error::Value(m) => ManifestError::Encode(m),
        })
    }

    /// Encode to a byte vector.
    pub fn to_cbor_vec(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.to_cbor(&mut buf)?;
        Ok(buf)
    }

    /// Decode CBOR. The `format_version` field is checked **before** the
    /// body is interpreted, so a manifest from a newer Carrova yields
    /// [`ManifestError::UnsupportedFormatVersion`] rather than a schema error.
    /// The result is fully [validated](Manifest::validate). Input is capped at
    /// [`DEFAULT_MAX_MANIFEST_BYTES`]; see [`Manifest::from_cbor_limited`].
    pub fn from_cbor(reader: impl Read) -> Result<Self> {
        Self::from_cbor_limited(reader, DEFAULT_MAX_MANIFEST_BYTES)
    }

    /// [`Manifest::from_cbor`] reading at most `max_len` bytes; larger input
    /// fails with [`ManifestError::TooLarge`]. Nesting depth is bounded too.
    /// The reader is consumed to its end: bytes after the manifest fail with
    /// [`ManifestError::TrailingBytes`].
    pub fn from_cbor_limited(reader: impl Read, max_len: u64) -> Result<Self> {
        let mut buf = Vec::new();
        reader
            .take(max_len.saturating_add(1))
            .read_to_end(&mut buf)?;
        if buf.len() as u64 > max_len {
            return Err(ManifestError::TooLarge { limit: max_len });
        }

        let peek: Peek = decode(&mut buf.as_slice()).map_err(|e| match e {
            // Well-formed CBOR without a usable u32 `format_version`.
            ciborium::de::Error::Semantic(..) => ManifestError::MissingFormatVersion,
            other => decode_error(other),
        })?;
        check_version(peek.format_version)?;

        let mut rest = buf.as_slice();
        let manifest: Self = decode(&mut rest).map_err(decode_error)?;
        if !rest.is_empty() {
            return Err(ManifestError::TrailingBytes);
        }
        manifest.validate()?;
        Ok(manifest)
    }
}

type DecodeError = ciborium::de::Error<std::io::Error>;

/// Typed decode straight from the in-memory buffer (no `Value` tree), with
/// the nesting limit. Advances `input` past the decoded item.
fn decode<T: DeserializeOwned>(input: &mut &[u8]) -> std::result::Result<T, DecodeError> {
    ciborium::de::from_reader_with_recursion_limit(input, MAX_NESTING)
}

fn decode_error(e: DecodeError) -> ManifestError {
    use ciborium::de::Error;
    match e {
        // Reading from memory: the only I/O error is running out of input.
        Error::Io(_) => ManifestError::Decode("truncated CBOR".into()),
        Error::Syntax(offset) => ManifestError::Decode(format!("malformed CBOR at byte {offset}")),
        Error::Semantic(_, msg) => ManifestError::Decode(msg),
        Error::RecursionLimitExceeded => {
            ManifestError::Decode(format!("nesting exceeds recursion limit of {MAX_NESTING}"))
        }
    }
}
