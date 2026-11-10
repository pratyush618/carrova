//! Key certificates, trust roots and chain verification.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::{Result, SignContext, SignError, Signature, SigningKey, VerifyingKey};

/// What a certified key may sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyRole {
    /// Signs recipe bundle indexes ([`SignContext::BundleIndex`]) only.
    BundleSigner,
}

impl KeyRole {
    /// Contexts a key of this role may sign: the one place this policy
    /// lives.
    #[must_use]
    pub const fn permitted(self) -> &'static [SignContext] {
        match self {
            Self::BundleSigner => &[SignContext::BundleIndex],
        }
    }

    /// Whether this role may sign under `context`.
    #[must_use]
    pub fn permits(self, context: SignContext) -> bool {
        self.permitted().contains(&context)
    }

    /// Stable one-byte code used in [`KeyCert::canonical_bytes`].
    const fn code(self) -> u8 {
        match self {
            Self::BundleSigner => 1,
        }
    }

    /// Stable lowercase name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BundleSigner => "bundle-signer",
        }
    }
}

impl fmt::Display for KeyRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

/// A root's statement that `key` may act as `role` from `not_before` to
/// `not_after` (unix seconds, both inclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyCert {
    /// The certified key.
    pub key: VerifyingKey,
    /// What the key may sign.
    pub role: KeyRole,
    /// First valid second (unix time).
    pub not_before: u64,
    /// Last valid second (unix time).
    pub not_after: u64,
}

/// Length of [`KeyCert::canonical_bytes`].
const CANONICAL_LEN: usize = 1 + 32 + 8 + 8;

/// Seconds in a day.
const DAY_SECS: u64 = 86_400;

/// Longest signer certificate, in whole days: signers are short-lived.
pub const MAX_CERT_VALIDITY_DAYS: u64 = 90;

/// How far issuers backdate `not_before`, so clients with a slow clock
/// accept a freshly issued certificate.
pub const CERT_CLOCK_SKEW: u64 = 3600;

/// Longest validity window (`not_after - not_before`, seconds) a
/// certificate may span: [`MAX_CERT_VALIDITY_DAYS`] plus
/// [`CERT_CLOCK_SKEW`]. Enforced when signing and in [`verify_chain`]; the
/// issuing tool derives its cap from the same constants.
pub const MAX_CERT_VALIDITY: u64 = MAX_CERT_VALIDITY_DAYS * DAY_SECS + CERT_CLOCK_SKEW;

impl KeyCert {
    /// Bytes the root signs: `role code (1) ‖ key (32) ‖ BE64 not_before ‖
    /// BE64 not_after`. Fixed length, so no field can bleed into another.
    #[must_use]
    pub fn canonical_bytes(&self) -> [u8; CANONICAL_LEN] {
        let mut out = [0u8; CANONICAL_LEN];
        out[0] = self.role.code();
        out[1..33].copy_from_slice(self.key.as_bytes());
        out[33..41].copy_from_slice(&self.not_before.to_be_bytes());
        out[41..].copy_from_slice(&self.not_after.to_be_bytes());
        out
    }

    /// Sign this certificate with `root`. Fails on an inverted validity
    /// window or one longer than [`MAX_CERT_VALIDITY`].
    pub fn sign(self, root: &SigningKey) -> Result<SignedKeyCert> {
        self.check_window()?;
        Ok(SignedKeyCert {
            cert: self,
            root: root.verifying_key(),
            sig: root.sign(SignContext::KeyCert, &self.canonical_bytes()),
        })
    }

    /// The window is ordered and no longer than [`MAX_CERT_VALIDITY`].
    fn check_window(&self) -> Result<()> {
        let secs = self
            .not_after
            .checked_sub(self.not_before)
            .ok_or(SignError::InvertedValidity)?;
        if secs > MAX_CERT_VALIDITY {
            return Err(SignError::ValidityTooLong {
                secs,
                max: MAX_CERT_VALIDITY,
            });
        }
        Ok(())
    }

    /// Whether `now` lies inside a well-formed validity window.
    fn check_valid_at(&self, now: u64) -> Result<()> {
        self.check_window()?;
        if now < self.not_before {
            return Err(SignError::NotYetValid {
                not_before: self.not_before,
                now,
            });
        }
        if now > self.not_after {
            return Err(SignError::Expired {
                not_after: self.not_after,
                now,
            });
        }
        Ok(())
    }
}

/// A [`KeyCert`] with the root key that signed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedKeyCert {
    /// The certificate.
    pub cert: KeyCert,
    /// Public key of the signing root (must be in the verifier's
    /// [`TrustRoots`]).
    pub root: VerifyingKey,
    /// Root signature over [`KeyCert::canonical_bytes`] under
    /// [`SignContext::KeyCert`].
    pub sig: Signature,
}

/// Root public keys a client trusts. Never empty; duplicates removed.
/// Serde form: a JSON array of hex keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<VerifyingKey>", into = "Vec<VerifyingKey>")]
pub struct TrustRoots(Vec<VerifyingKey>);

impl TrustRoots {
    /// Trust exactly `keys` (in order, without duplicates).
    pub fn new(keys: impl IntoIterator<Item = VerifyingKey>) -> Result<Self> {
        let mut out: Vec<VerifyingKey> = Vec::new();
        for key in keys {
            if !out.contains(&key) {
                out.push(key);
            }
        }
        if out.is_empty() {
            return Err(SignError::NoTrustRoots);
        }
        Ok(Self(out))
    }

    /// Whether `key` is a trusted root.
    #[must_use]
    pub fn contains(&self, key: &VerifyingKey) -> bool {
        self.0.contains(key)
    }

    /// The trusted keys.
    pub fn iter(&self) -> impl Iterator<Item = &VerifyingKey> {
        self.0.iter()
    }
}

impl TryFrom<Vec<VerifyingKey>> for TrustRoots {
    type Error = SignError;

    fn try_from(keys: Vec<VerifyingKey>) -> Result<Self> {
        Self::new(keys)
    }
}

impl From<TrustRoots> for Vec<VerifyingKey> {
    fn from(roots: TrustRoots) -> Self {
        roots.0
    }
}

/// Verify `sig` over `payload` under `context` by the key certified in
/// `cert`, and the certificate itself against `roots` at time `now` (unix
/// seconds, supplied by the caller).
///
/// Checks, in order: the root is trusted, the root's certificate
/// signature, the validity window (at most [`MAX_CERT_VALIDITY`] long and
/// containing `now`), the role may sign `context`, the payload signature. All signature checks are strict.
pub fn verify_chain(
    roots: &TrustRoots,
    cert: &SignedKeyCert,
    context: SignContext,
    payload: &[u8],
    sig: &Signature,
    now: u64,
) -> Result<()> {
    if !roots.contains(&cert.root) {
        return Err(SignError::UnknownRoot(cert.root));
    }
    cert.root
        .verify(
            SignContext::KeyCert,
            &cert.cert.canonical_bytes(),
            &cert.sig,
        )
        .map_err(|_| SignError::BadCertSignature)?;
    cert.cert.check_valid_at(now)?;
    if !cert.cert.role.permits(context) {
        return Err(SignError::WrongRole {
            role: cert.cert.role,
            context,
        });
    }
    cert.cert.key.verify(context, payload, sig)
}
