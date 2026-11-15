//! Confirmation keys and the TLS channel binding they are bound to.

use carrova_crypto::identity::DevicePublic;
use carrova_crypto::pairing::{
    CertHash, ChannelBinding, Confirmation, EXPORTER_LABEL, EXPORTER_LEN, KnownSecret,
    PairingError, PakeRole, RESUME_SECRET_LEN, ResumeSecret, SessionKeys,
};

use super::PeerIdentity;
use super::identify::Proof;
use crate::error::{Result, SetupFailure, TransportError};
use crate::tls::{self, Identity};

/// Keys that produce and check this session's confirmations.
pub(super) enum Keys<'a> {
    Pake(SessionKeys),
    Resume(&'a ResumeSecret),
    /// A link: the secret shared with this known peer.
    Link(&'a PeerIdentity),
}

impl Keys<'_> {
    pub(super) fn confirm(&self, role: PakeRole, binding: &ChannelBinding) -> Result<Confirmation> {
        Ok(match self {
            Self::Pake(keys) => keys.confirm(role, binding)?,
            Self::Resume(secret) => secret.prove(role, binding)?,
            Self::Link(peer) => peer.secret.prove(role, binding)?,
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
            Self::Link(known) => known.secret.verify(peer, binding, c),
        }
        .map_err(|_| self.proof(None).failure())
    }

    /// The secret for recognising a remembered device: code pairing (new)
    /// or link (the one proven).
    pub(super) fn known_secret(&self) -> Option<KnownSecret> {
        match self {
            Self::Pake(keys) => Some(keys.known_secret()),
            Self::Resume(_) => None,
            Self::Link(peer) => Some(peer.secret.clone()),
        }
    }

    /// What the peer's `Identify` must prove: a link, by the known peer's
    /// key; a pairing, by any key (or `pin`, from a scanned QR code).
    pub(super) fn proof<'b>(&'b self, pin: Option<&'b DevicePublic>) -> Proof<'b> {
        match self {
            Self::Link(peer) => Proof::Link(&peer.public),
            Self::Pake(_) | Self::Resume(_) => Proof::Pairing(pin),
        }
    }

    pub(super) fn resume_secret(&self) -> Result<ResumeSecret> {
        Ok(match self {
            Self::Pake(keys) => keys.resume_secret(),
            Self::Resume(secret) => (*secret).clone(),
            // A link is redialled, never resumed: a random secret no peer
            // shares, so a resume with it can only fail.
            Self::Link(_) => {
                let mut bytes = [0u8; RESUME_SECRET_LEN];
                getrandom::fill(&mut bytes).map_err(|_| PairingError::Rng)?;
                ResumeSecret::from_bytes(bytes)
            }
        })
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
