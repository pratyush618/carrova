//! The container format: magic, version, record tags and the plaintext
//! [`ArchiveHeader`].

use carrova_crypto::{KdfParams, NONCE_PREFIX_LEN, NoncePrefix, SALT_LEN, Salt};

use crate::error::{ArchiveError, Result};

/// File magic.
pub const MAGIC: [u8; 8] = *b"CARROVA\0";
/// Archive container version written by this build.
pub const ARCHIVE_VERSION: u16 = 1;
/// Serialized header length in bytes.
pub const HEADER_LEN: usize = 8 + 2 + 2 + 4 + 4 + 4 + SALT_LEN + NONCE_PREFIX_LEN;
/// Largest chunk accepted in an archive: the manifest-wide limit.
pub use carrova_manifest::MAX_CHUNK_LEN;

pub(crate) const TAG_END: u8 = 0;
pub(crate) const TAG_MANIFEST: u8 = 1;
pub(crate) const TAG_CHUNK: u8 = 2;

/// Decoded plaintext header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveHeader {
    pub version: u16,
    pub flags: u16,
    pub kdf: KdfParams,
    pub salt: carrova_crypto::Salt,
    pub nonce: carrova_crypto::NoncePrefix,
}

impl ArchiveHeader {
    /// Serialize (explicit little-endian fields). These bytes are the AAD.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        let mut at = 0;
        let mut put = |b: &[u8]| {
            out[at..at + b.len()].copy_from_slice(b);
            at += b.len();
        };
        put(&MAGIC);
        put(&self.version.to_le_bytes());
        put(&self.flags.to_le_bytes());
        put(&self.kdf.m_kib.to_le_bytes());
        put(&self.kdf.t.to_le_bytes());
        put(&self.kdf.p.to_le_bytes());
        put(&self.salt.0);
        put(&self.nonce.0);
        out
    }

    /// Parse and check magic, version and flags. KDF bounds are checked by
    /// key derivation ([`KdfParams::validate`]).
    pub fn from_bytes(b: &[u8; HEADER_LEN]) -> Result<Self> {
        let mut r = &b[..];
        let mut take = |n: usize| {
            let (head, rest) = r.split_at(n);
            r = rest;
            head
        };
        let u16_at = |s: &[u8]| u16::from_le_bytes([s[0], s[1]]);
        let u32_at = |s: &[u8]| u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        if take(8) != MAGIC {
            return Err(ArchiveError::BadMagic);
        }
        let version = u16_at(take(2));
        if version != ARCHIVE_VERSION {
            return Err(ArchiveError::UnsupportedVersion(version));
        }
        let flags = u16_at(take(2));
        if flags != 0 {
            return Err(ArchiveError::UnsupportedFlags(flags));
        }
        let kdf = KdfParams {
            m_kib: u32_at(take(4)),
            t: u32_at(take(4)),
            p: u32_at(take(4)),
        };
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(take(SALT_LEN));
        let mut nonce = [0u8; NONCE_PREFIX_LEN];
        nonce.copy_from_slice(take(NONCE_PREFIX_LEN));
        Ok(Self {
            version,
            flags,
            kdf,
            salt: Salt(salt),
            nonce: NoncePrefix(nonce),
        })
    }
}
