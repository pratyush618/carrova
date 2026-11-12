//! Confirmation keys and the TLS channel binding they are bound to.

use carrova_crypto::pairing::{
    CertHash, ChannelBinding, Confirmation, EXPORTER_LABEL, EXPORTER_LEN, KnownSecret, PakeRole,
    ResumeSecret, SessionKeys,
};

use crate::error::{Result, SetupFailure, TransportError};
use crate::tls::{self, Identity};

/// Keys that produce and check this session's confirmations.
pub(super) enum Keys<'a> {
    Pake(SessionKeys),
    Resume(&'a ResumeSecret),
}

impl Keys<'_> {
    pub(super) fn confirm(&self, role: PakeRole, binding: &ChannelBinding) -> Result<Confirmation> {
        Ok(match self {
            Self::Pake(keys) => keys.confirm(role, binding)?,
            Self::Resume(secret) => secret.prove(role, binding)?,
        })
    }

    pub(super) fn verify(
        &self,
        peer: PakeRole,
        binding: &ChannelBinding,
        c: &Confirmation,
    ) -> Result<()> {
        match self {
            Self::Pake(keys) => keys.verify(peer, binding, c),
            Self::Resume(secret) => secret.verify(peer, binding, c),
        }
        .map_err(|_| TransportError::PairingFailed)
    }

    /// The secret for recognising a remembered device: code pairing only.
    pub(super) fn known_secret(&self) -> Option<KnownSecret> {
        match self {
            Self::Pake(keys) => Some(keys.known_secret()),
            Self::Resume(_) => None,
        }
    }

    pub(super) fn resume_secret(&self) -> ResumeSecret {
        match self {
            Self::Pake(keys) => keys.resume_secret(),
            Self::Resume(secret) => (*secret).clone(),
        }
    }
}

/// TLS-exporter + certificate binding of `conn`, and the peer's cert hash.
pub(crate) fn binding(
    conn: &quinn::Connection,
    own: &Identity,
    role: PakeRole,
) -> Result<(ChannelBinding, CertHash)> {
    let mut exporter = [0u8; EXPORTER_LEN];
    conn.export_keying_material(&mut exporter, EXPORTER_LABEL, b"")
        .map_err(|_| TransportError::from(SetupFailure::ExporterUnavailable))?;
    let peer = tls::peer_cert(conn)?;
    let binding = match role {
        PakeRole::Sender => ChannelBinding::new(exporter, own.cert_der(), &peer),
        PakeRole::Receiver => ChannelBinding::new(exporter, &peer, own.cert_der()),
    };
    Ok((binding, CertHash::of(&peer)))
}
