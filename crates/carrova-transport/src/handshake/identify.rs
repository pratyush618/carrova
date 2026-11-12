//! Device identity exchange on a code-paired connection (ADR 0001 §3):
//! each side signs this connection's channel binding with its device key.

use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::{ChannelBinding, KnownSecret, PakeRole};

use super::io::unexpected;
use crate::error::{Result, TransportError};
use crate::wire::Handshake;

/// A code-paired peer whose device identity was exchanged both ways, and
/// the secret both now share for recognising each other later.
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    /// The peer's identity key.
    pub public: DevicePublic,
    /// The secret derived from this pairing (never logged).
    pub secret: KnownSecret,
}

/// This side's `Identify` frame, signed as `role`.
pub(super) fn proof(device: &DeviceKey, role: PakeRole, binding: &ChannelBinding) -> Handshake {
    Handshake::Identify {
        public: *device.public().as_bytes(),
        sig: device.sign_binding(role, binding),
    }
}

/// The peer's identity from its `Identify` frame, signed as `peer_role`.
/// Any bad proof fails the pairing; only the failure kind is logged.
pub(super) fn check(
    device: &DeviceKey,
    peer_role: PakeRole,
    binding: &ChannelBinding,
    frame: &Handshake,
) -> Result<DevicePublic> {
    let Handshake::Identify { public, sig } = frame else {
        return Err(unexpected("identify"));
    };
    DevicePublic::from_bytes(public)
        .and_then(|peer| {
            device
                .verify_peer(&peer, peer_role, binding, sig)
                .map(|()| peer)
        })
        .map_err(|error| {
            tracing::warn!(%error, "peer's device identity rejected");
            TransportError::PairingFailed
        })
}
