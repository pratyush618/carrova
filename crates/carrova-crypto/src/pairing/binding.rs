//! Channel binding: certificate hashes and the TLS exporter.

use std::fmt;
use std::str::FromStr;

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::{
    BINDING_LEN, CERT_HASH_LEN, EXPORTER_LEN, PairingError, Result, hex_decode, hex_encode,
};

/// SHA-256 of a peer's DER certificate. Identifies the peer within one
/// session (certificates are per process, so it changes on restart).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CertHash([u8; CERT_HASH_LEN]);

impl CertHash {
    /// Hash of `der` (the certificate as sent in the TLS handshake).
    #[must_use]
    pub fn of(der: &[u8]) -> Self {
        Self(Sha256::digest(der).into())
    }

    /// A hash received or stored elsewhere (e.g. a persisted session).
    #[must_use]
    pub const fn from_bytes(bytes: [u8; CERT_HASH_LEN]) -> Self {
        Self(bytes)
    }

    /// Raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; CERT_HASH_LEN] {
        &self.0
    }
}

impl fmt::Display for CertHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex_encode(&self.0, f)
    }
}

impl FromStr for CertHash {
    type Err = PairingError;

    fn from_str(s: &str) -> Result<Self> {
        hex_decode(s, "certificate hash").map(Self)
    }
}

serde_via_str!(CertHash);

/// Binds confirmation MACs to one TLS session and both certificates:
/// `exporter ‖ SHA-256(sender cert) ‖ SHA-256(receiver cert)`.
#[derive(Clone)]
pub struct ChannelBinding(Zeroizing<[u8; BINDING_LEN]>);

impl ChannelBinding {
    /// `exporter` is the TLS exporter output for [`EXPORTER_LABEL`](super::EXPORTER_LABEL)
    /// ([`EXPORTER_LEN`] bytes, empty context); certificates are DER.
    #[must_use]
    pub fn new(exporter: [u8; EXPORTER_LEN], sender_cert: &[u8], receiver_cert: &[u8]) -> Self {
        let exporter = Zeroizing::new(exporter);
        let mut out = Zeroizing::new([0u8; BINDING_LEN]);
        out[..EXPORTER_LEN].copy_from_slice(&*exporter);
        out[EXPORTER_LEN..EXPORTER_LEN + CERT_HASH_LEN]
            .copy_from_slice(CertHash::of(sender_cert).as_bytes());
        out[EXPORTER_LEN + CERT_HASH_LEN..].copy_from_slice(CertHash::of(receiver_cert).as_bytes());
        Self(out)
    }

    /// Serialized binding (MAC input after the role identity).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; BINDING_LEN] {
        &self.0
    }
}

impl fmt::Debug for ChannelBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChannelBinding(<redacted>)")
    }
}
