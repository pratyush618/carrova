use std::fmt;
use std::str::FromStr;

use data_encoding::{HEXLOWER, HEXLOWER_PERMISSIVE};
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ManifestError;

/// A BLAKE3 digest (32 bytes).
///
/// Displays / parses as lowercase hex. Serialises as a CBOR byte string
/// (hex string for human-readable formats such as JSON).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Blake3Hash(pub [u8; 32]);

/// Hash identifying a content-addressed chunk.
pub type ChunkHash = Blake3Hash;

impl Blake3Hash {
    /// Hash `data` in one shot.
    #[must_use]
    pub fn of(data: &[u8]) -> Self {
        Self(*blake3::hash(data).as_bytes())
    }

    /// Raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<blake3::Hash> for Blake3Hash {
    fn from(h: blake3::Hash) -> Self {
        Self(*h.as_bytes())
    }
}

impl fmt::Display for Blake3Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&HEXLOWER.encode_display(&self.0), f)
    }
}

impl fmt::Debug for Blake3Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blake3Hash({self})")
    }
}

impl FromStr for Blake3Hash {
    type Err = ManifestError;

    /// Exactly 64 hex digits, either case.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut out = [0u8; 32];
        if s.len() != 2 * out.len() {
            return Err(ManifestError::InvalidHashHex);
        }
        HEXLOWER_PERMISSIVE
            .decode_mut(s.as_bytes(), &mut out)
            .map_err(|_| ManifestError::InvalidHashHex)?;
        Ok(Self(out))
    }
}

impl Serialize for Blake3Hash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if s.is_human_readable() {
            s.collect_str(self)
        } else {
            s.serialize_bytes(&self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Blake3Hash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;

        impl<'de> Visitor<'de> for V {
            type Value = Blake3Hash;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("32 bytes or a 64-char hex string")
            }

            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                <[u8; 32]>::try_from(v)
                    .map(Blake3Hash)
                    .map_err(|_| E::invalid_length(v.len(), &self))
            }

            fn visit_byte_buf<E: de::Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                self.visit_bytes(&v)
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                v.parse()
                    .map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut out = [0u8; 32];
                for (i, slot) in out.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                if seq.next_element::<u8>()?.is_some() {
                    return Err(de::Error::invalid_length(33, &self));
                }
                Ok(Blake3Hash(out))
            }
        }

        if d.is_human_readable() {
            d.deserialize_str(V)
        } else {
            d.deserialize_bytes(V)
        }
    }
}
