//! Key and signature newtypes over `ed25519-dalek`.

use std::fmt;
use std::str::FromStr;

use ed25519_dalek::Signer as _;
use zeroize::Zeroizing;

use super::{Result, SignContext, SignError, hex_decode, hex_encode};

/// Length of a secret key seed.
pub const SECRET_KEY_LEN: usize = ed25519_dalek::SECRET_KEY_LENGTH;
const PUBLIC_KEY_LEN: usize = ed25519_dalek::PUBLIC_KEY_LENGTH;
const SIGNATURE_LEN: usize = ed25519_dalek::SIGNATURE_LENGTH;

/// Ed25519 secret key. Zeroized on drop (dalek's `ZeroizeOnDrop`);
/// `Debug` prints `SigningKey(***)`.
pub struct SigningKey(ed25519_dalek::SigningKey);

impl SigningKey {
    /// Fresh key from a 32-byte seed drawn from the OS RNG.
    pub fn generate() -> Result<Self> {
        let mut seed = Zeroizing::new([0u8; SECRET_KEY_LEN]);
        getrandom::fill(seed.as_mut_slice()).map_err(|_| SignError::Rng)?;
        Ok(Self::from_seed(&seed))
    }

    /// Key from its 32-byte seed (copied; the caller zeroizes its own copy).
    #[must_use]
    pub fn from_seed(seed: &[u8; SECRET_KEY_LEN]) -> Self {
        Self(ed25519_dalek::SigningKey::from_bytes(seed))
    }

    /// Parse a seed persisted with [`Self::to_secret_hex`].
    pub fn from_secret_hex(hex: &str) -> Result<Self> {
        let seed = Zeroizing::new(hex_decode::<SECRET_KEY_LEN>(hex.trim(), "secret key")?);
        Ok(Self::from_seed(&seed))
    }

    /// Seed as lowercase hex, for persisting the key only.
    #[must_use]
    pub fn to_secret_hex(&self) -> Zeroizing<String> {
        Zeroizing::new(data_encoding::HEXLOWER.encode(self.0.as_bytes()))
    }

    /// Public half of this key.
    #[must_use]
    pub fn verifying_key(&self) -> VerifyingKey {
        VerifyingKey(self.0.verifying_key().to_bytes())
    }

    /// Sign `payload` under `context` (the signed bytes are
    /// `context.tag() ‖ payload`).
    #[must_use]
    pub fn sign(&self, context: SignContext, payload: &[u8]) -> Signature {
        Signature(self.0.sign(&context.message(payload)))
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SigningKey(***)")
    }
}

/// Ed25519 public key. Text and serde form: 64 hex digits.
///
/// Holds the 32-byte compressed encoding, checked to decode to a curve
/// point on construction (so the type stays small and `Copy`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct VerifyingKey([u8; PUBLIC_KEY_LEN]);

impl VerifyingKey {
    /// Key from its 32-byte compressed encoding.
    pub fn from_bytes(bytes: &[u8; PUBLIC_KEY_LEN]) -> Result<Self> {
        Self::decode(bytes).map(|_| Self(*bytes))
    }

    /// 32-byte compressed encoding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LEN] {
        &self.0
    }

    /// Strictly verify `signature` over `context.tag() ‖ payload`.
    pub fn verify(
        &self,
        context: SignContext,
        payload: &[u8],
        signature: &Signature,
    ) -> Result<()> {
        Self::decode(&self.0)?
            .verify_strict(&context.message(payload), &signature.0)
            .map_err(|_| SignError::BadSignature)
    }

    fn decode(bytes: &[u8; PUBLIC_KEY_LEN]) -> Result<ed25519_dalek::VerifyingKey> {
        ed25519_dalek::VerifyingKey::from_bytes(bytes).map_err(|_| SignError::InvalidKey)
    }
}

impl fmt::Display for VerifyingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex_encode(self.as_bytes(), f)
    }
}

impl fmt::Debug for VerifyingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VerifyingKey({self})")
    }
}

impl FromStr for VerifyingKey {
    type Err = SignError;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_bytes(&hex_decode(s.trim(), "public key")?)
    }
}

serde_via_str!(VerifyingKey);

/// Ed25519 signature. Text and serde form: 128 hex digits.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Signature(ed25519_dalek::Signature);

impl Signature {
    /// Signature from its 64-byte encoding (checked when verifying).
    #[must_use]
    pub fn from_bytes(bytes: &[u8; SIGNATURE_LEN]) -> Self {
        Self(ed25519_dalek::Signature::from_bytes(bytes))
    }

    /// 64-byte encoding.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; SIGNATURE_LEN] {
        self.0.to_bytes()
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex_encode(&self.to_bytes(), f)
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signature({self})")
    }
}

impl FromStr for Signature {
    type Err = SignError;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self::from_bytes(&hex_decode(s.trim(), "signature")?))
    }
}

serde_via_str!(Signature);
