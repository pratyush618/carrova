//! Content addressing: blob hashes, blob kinds and asset names.

use std::fmt;
use std::str::FromStr;

use data_encoding::HEXLOWER;
use serde::{Deserialize, Serialize};

use crate::{BundleError, Result};

/// BLAKE3 hash of a blob. Text form: exactly 64 lowercase hex digits
/// (anything else is rejected, so a hash is always a safe file-name stem).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlobHash([u8; 32]);

impl BlobHash {
    /// Hash of `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }
}

impl fmt::Display for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&HEXLOWER.encode(&self.0))
    }
}

impl fmt::Debug for BlobHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BlobHash({self})")
    }
}

impl FromStr for BlobHash {
    type Err = BundleError;

    fn from_str(s: &str) -> Result<Self> {
        let malformed = || BundleError::MalformedHash(s.chars().take(80).collect());
        if !is_hash_text(s) {
            return Err(malformed());
        }
        let mut out = [0u8; 32];
        HEXLOWER
            .decode_mut(s.as_bytes(), &mut out)
            .map_err(|_| malformed())?;
        Ok(Self(out))
    }
}

impl Serialize for BlobHash {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for BlobHash {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Whether `s` is a [`BlobHash`] in text form: exactly 64 lowercase hex
/// digits (`const`, so build-time pins are checked by the compiler too).
pub(crate) const fn is_hash_text(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 64 {
        return false;
    }
    let mut i = 0;
    while i < bytes.len() {
        if !matches!(bytes[i], b'0'..=b'9' | b'a'..=b'f') {
            return false;
        }
        i += 1;
    }
    true
}

/// What a blob holds; fixes its file extension and size limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlobKind {
    /// A recipe YAML file.
    Recipe,
    /// The precompiled [`crate::DetectIndex`].
    Detect,
    /// A plugin's `plugin.yaml`.
    PluginManifest,
    /// A plugin's WebAssembly module.
    Plugin,
}

impl BlobKind {
    /// File extension (without the dot).
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Recipe => "yaml",
            Self::Detect => "json",
            Self::PluginManifest => "plugin.yaml",
            Self::Plugin => "wasm",
        }
    }

    /// Largest accepted blob of this kind, in bytes.
    #[must_use]
    pub const fn max_size(self) -> u64 {
        match self {
            Self::Recipe => 256 * 1024,
            Self::Detect => 4 * 1024 * 1024,
            Self::PluginManifest => 16 * 1024,
            Self::Plugin => carrova_plugin::MAX_WASM_BYTES,
        }
    }
}

/// Hash and size of a blob, as recorded in the signed index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobRef {
    /// BLAKE3 of the bytes.
    pub hash: BlobHash,
    /// Length in bytes.
    pub size: u64,
}

impl BlobRef {
    /// Reference to `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self {
            hash: BlobHash::of(bytes),
            size: bytes.len() as u64,
        }
    }

    /// Whether `bytes` are exactly this blob.
    #[must_use]
    pub fn matches(&self, bytes: &[u8]) -> bool {
        bytes.len() as u64 == self.size && BlobHash::of(bytes) == self.hash
    }
}

/// A file of a bundle. The only way to name one: blob names are built
/// from validated hashes, never from strings in the index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Asset {
    /// `index.json`.
    Index,
    /// `index.sig.json`.
    Signature,
    /// `<hash>.<ext>`.
    Blob(BlobKind, BlobHash),
}

impl Asset {
    /// File name of the index.
    pub const INDEX_FILE: &str = "index.json";
    /// File name of the index signature.
    pub const SIGNATURE_FILE: &str = "index.sig.json";

    /// File name inside a bundle directory or release.
    #[must_use]
    pub fn file_name(&self) -> String {
        match self {
            Self::Index => Self::INDEX_FILE.to_owned(),
            Self::Signature => Self::SIGNATURE_FILE.to_owned(),
            Self::Blob(kind, hash) => format!("{hash}.{}", kind.extension()),
        }
    }

    /// Largest accepted size of this asset, in bytes.
    #[must_use]
    pub const fn max_size(&self) -> u64 {
        match self {
            Self::Index => 4 * 1024 * 1024,
            Self::Signature => 16 * 1024,
            Self::Blob(kind, _) => kind.max_size(),
        }
    }
}
