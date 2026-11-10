//! Signed index: `index.json` bytes plus `index.sig.json`, verified.

use carrova_crypto::sign::{SignContext, Signature, SignedKeyCert, SigningKey, TrustRoots};
use serde::{Deserialize, Serialize};

use crate::{BundleError, BundleIndex, BundleVersion, Result};

/// Contents of `index.sig.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexSignature {
    /// The signer's certificate, issued by a trusted root.
    pub cert: SignedKeyCert,
    /// Signer's signature over the raw `index.json` bytes under
    /// [`SignContext::BundleIndex`].
    pub sig: Signature,
}

impl IndexSignature {
    /// Sign raw index bytes with `signer`, certified by `cert`.
    pub fn create(index_bytes: &[u8], signer: &SigningKey, cert: SignedKeyCert) -> Result<Self> {
        if signer.verifying_key() != cert.cert.key {
            return Err(BundleError::SignerMismatch);
        }
        Ok(Self {
            cert,
            sig: signer.sign(SignContext::BundleIndex, index_bytes),
        })
    }

    /// Pretty JSON for `index.sig.json`.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        serde_json::to_vec_pretty(self).map_err(BundleError::json("index signature"))
    }
}

/// The time at which the signer certificate must be valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyTime {
    /// The caller's current time (unix seconds): remote updates and
    /// verification of a freshly built bundle.
    Now(u64),
    /// The index's own `created` time: bundles shipped inside the app,
    /// whose integrity the app package already guarantees and which may
    /// outlive the short-lived signer certificate.
    SignedAt,
}

/// An index whose signature chain has been verified. Only
/// [`SignedIndex::verify`] creates one, so holding it proves the check.
#[derive(Debug, Clone)]
pub struct SignedIndex {
    index: BundleIndex,
    index_bytes: Vec<u8>,
    sig_bytes: Vec<u8>,
}

impl SignedIndex {
    /// Verify `index_bytes` against `sig_bytes` and `roots`, then parse
    /// and validate the index.
    pub fn verify(
        index_bytes: Vec<u8>,
        sig_bytes: Vec<u8>,
        roots: &TrustRoots,
        time: VerifyTime,
    ) -> Result<Self> {
        let sig: IndexSignature =
            serde_json::from_slice(&sig_bytes).map_err(BundleError::json("index signature"))?;
        let check = |now| {
            carrova_crypto::sign::verify_chain(
                roots,
                &sig.cert,
                SignContext::BundleIndex,
                &index_bytes,
                &sig.sig,
                now,
            )
        };
        let index = match time {
            VerifyTime::Now(now) => {
                check(now)?;
                BundleIndex::parse(&index_bytes)?
            }
            VerifyTime::SignedAt => {
                let index = BundleIndex::parse(&index_bytes)?;
                check(index.created)?;
                index
            }
        };
        tracing::debug!(version = %index.version, signer = %sig.cert.cert.key, "bundle index verified");
        Ok(Self {
            index,
            index_bytes,
            sig_bytes,
        })
    }

    /// The verified index.
    #[must_use]
    pub fn index(&self) -> &BundleIndex {
        &self.index
    }

    /// Its version.
    #[must_use]
    pub fn version(&self) -> BundleVersion {
        self.index.version
    }

    /// Raw `index.json` bytes (what was signed).
    #[must_use]
    pub fn index_bytes(&self) -> &[u8] {
        &self.index_bytes
    }

    /// Raw `index.sig.json` bytes.
    #[must_use]
    pub fn sig_bytes(&self) -> &[u8] {
        &self.sig_bytes
    }
}
