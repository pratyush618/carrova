//! Device identity exchange: on a code-paired connection (ADR 0001 §3) or
//! a link (ADR 0002 §3), each side signs this connection's channel
//! binding with its device key.

use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::{ChannelBinding, KnownSecret, PakeRole};

use super::io::unexpected;
use crate::error::{Result, TransportError};
use crate::wire::Handshake;

/// A code-paired or linked peer whose device identity was exchanged both
/// ways, and the secret both share for recognising each other.
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    /// The peer's identity key.
    pub public: DevicePublic,
    /// The secret derived from the pairing (never logged).
    pub secret: KnownSecret,
}

/// What an `Identify` frame proves, so how it is signed and checked.
#[derive(Debug, Clone, Copy)]
pub(super) enum Proof<'a> {
    /// A code pairing: any key, or exactly the one pinned by a scanned QR
    /// code.
    Pairing(Option<&'a DevicePublic>),
    /// A link: exactly this known peer's key, under its own signing
    /// context.
    Link(&'a DevicePublic),
}

impl Proof<'_> {
    /// How a bad proof fails the handshake.
    pub(super) const fn failure(self) -> TransportError {
        match self {
            Self::Pairing(_) => TransportError::PairingFailed,
            Self::Link(_) => TransportError::LinkFailed,
        }
    }

    /// How a valid proof by another key than the expected one fails it.
    const fn mismatch(self) -> TransportError {
        match self {
            Self::Pairing(_) => TransportError::KeyMismatch,
            Self::Link(_) => TransportError::LinkFailed,
        }
    }
}

/// This side's `Identify` frame, signed as `role`.
pub(super) fn proof(
    device: &DeviceKey,
    role: PakeRole,
    binding: &ChannelBinding,
    kind: Proof<'_>,
) -> Handshake {
    let sig = match kind {
        Proof::Pairing(_) => device.sign_binding(role, binding),
        Proof::Link(_) => device.sign_link(role, binding),
    };
    Handshake::Identify {
        public: *device.public().as_bytes(),
        sig,
    }
}

/// The peer's identity from its `Identify` frame, signed as `peer_role`
/// with the key `kind` expects. Any bad proof fails the handshake; only
/// the failure kind is logged.
pub(super) fn check(
    device: &DeviceKey,
    peer_role: PakeRole,
    binding: &ChannelBinding,
    frame: &Handshake,
    kind: Proof<'_>,
) -> Result<DevicePublic> {
    let Handshake::Identify { public, sig } = frame else {
        return Err(unexpected("identify"));
    };
    let peer = DevicePublic::from_bytes(public)
        .and_then(|peer| {
            match kind {
                Proof::Pairing(_) => device.verify_peer(&peer, peer_role, binding, sig),
                Proof::Link(_) => device.verify_link_peer(&peer, peer_role, binding, sig),
            }
            .map(|()| peer)
        })
        .map_err(|error| {
            tracing::warn!(%error, "peer's device identity rejected");
            kind.failure()
        })?;
    let expected = match kind {
        Proof::Pairing(pin) => pin,
        Proof::Link(known) => Some(known),
    };
    if expected.is_some_and(|key| *key != peer) {
        tracing::warn!("peer's key differs from the expected one");
        return Err(kind.mismatch());
    }
    Ok(peer)
}
