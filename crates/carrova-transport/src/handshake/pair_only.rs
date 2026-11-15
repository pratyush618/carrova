//! Pair-only (ADR 0002 §1): after `Established` both sides exchange
//! `PeerInfo`; the sender confirms with `Linked` once its user accepted
//! and it stored the peer, the receiver returns on `Linked` (waiting up to
//! [`PAIR_CONFIRM_TIMEOUT`]), and both close.

use std::time::Duration;

use carrova_manifest::MAX_DEVICE_NAME;
use carrova_platform::OsKind;
use quinn::SendStream;
use tokio::time::timeout;

use super::io::{failure_code, unexpected, write};
use super::{Authenticated, PeerIdentity};
use crate::connection::flush;
use crate::error::{ProtocolViolation, Result, SetupFailure, TransportError};
use crate::runtime::runtime;
use crate::tuning::{HANDSHAKE_TIMEOUT, PAIR_CONFIRM_TIMEOUT};
use crate::wire::{CloseCode, Handshake};

/// What this side tells its pair-only peer about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    /// Display name, at most
    /// [`MAX_DEVICE_NAME`](carrova_manifest::MAX_DEVICE_NAME) characters
    /// (pass a cleaned name: it is shown on the other device).
    pub name: String,
    /// This device's OS family.
    pub os: OsKind,
}

/// A device paired without a transfer: its proven identity, the secret
/// now shared with it, and what it says about itself.
#[derive(Debug, Clone)]
pub struct PairedPeer {
    /// The peer's identity key and the shared secret.
    pub identity: PeerIdentity,
    /// The peer's display name (untrusted; clean before display).
    pub name: String,
    /// The peer's OS family (untrusted; for display).
    pub os: OsKind,
}

/// The sender's pair-only result before the receiver is told: store
/// [`Self::peer`], then [`Self::confirm`]. Dropping it unconfirmed fails
/// the pairing on both sides (the receiver stores nothing).
///
/// The user may check the peer's key in between, but confirm within
/// [`PAIR_CONFIRM_TIMEOUT`] (60 s) of the exchange: then the receiver
/// gives up, and [`Self::confirm`] fails (do not keep the stored peer).
#[must_use = "the receiver learns of the pairing only on confirm"]
#[derive(Debug)]
pub struct PendingPair {
    /// The peer to store before confirming.
    pub peer: PairedPeer,
    _endpoint: quinn::Endpoint,
    conn: quinn::Connection,
    send: SendStream,
    /// How dropping closes the connection.
    close: CloseCode,
}

impl PendingPair {
    /// Tell the receiver the pairing stands (`Linked`) and close the
    /// connection normally. Call only after [`Self::peer`] is stored.
    ///
    /// Fails if the receiver had already given up (it closes with a
    /// failure code, not normally): then do not keep the stored peer.
    pub fn confirm(mut self) -> Result<()> {
        runtime()?.block_on(write(&mut self.send, &[Handshake::Linked]))?;
        flush(&mut self.send);
        match self
            .conn
            .close_reason()
            .map(|r| TransportError::connection(&r))
        {
            // Closed normally (or lost after `Linked` went out): linked.
            None | Some(TransportError::PeerLost) => {}
            Some(e) => return Err(e),
        }
        self.close = CloseCode::Normal;
        Ok(())
    }
}

impl Drop for PendingPair {
    fn drop(&mut self) {
        if self.close == CloseCode::Normal {
            flush(&mut self.send);
        }
        self.close.close(&self.conn);
    }
}

/// Sender, after `Established`: exchange `PeerInfo`. `Linked` waits for
/// the caller ([`PendingPair::confirm`]).
pub(crate) async fn offer(
    endpoint: quinn::Endpoint,
    mut auth: Authenticated,
    me: &PeerInfo,
) -> Result<PendingPair> {
    match bounded(HANDSHAKE_TIMEOUT, exchange(&mut auth, me)).await {
        Ok(peer) => Ok(PendingPair {
            peer,
            _endpoint: endpoint,
            conn: auth.conn,
            send: auth.send,
            close: CloseCode::PairingFailed,
        }),
        Err(e) => {
            failure_code(&e).close(&auth.conn);
            Err(e)
        }
    }
}

/// Receiver, after `Established`: exchange `PeerInfo` (within
/// `HANDSHAKE_TIMEOUT`), wait up to [`PAIR_CONFIRM_TIMEOUT`] for the
/// sender's `Linked` (its user is checking our key), then close.
pub(crate) async fn receive(mut auth: Authenticated, me: &PeerInfo) -> Result<PairedPeer> {
    let result = async {
        let peer = bounded(HANDSHAKE_TIMEOUT, exchange(&mut auth, me)).await?;
        match bounded(PAIR_CONFIRM_TIMEOUT, auth.recv.expect_frame()).await? {
            Handshake::Linked => Ok(peer),
            _ => Err(unexpected("linked")),
        }
    }
    .await;
    match &result {
        Ok(_) => CloseCode::Normal,
        Err(e) => failure_code(e),
    }
    .close(&auth.conn);
    result
}

async fn bounded<T>(limit: Duration, fut: impl Future<Output = Result<T>>) -> Result<T> {
    timeout(limit, fut)
        .await
        .unwrap_or(Err(TransportError::Timeout))
}

/// Send our `PeerInfo` and read the peer's.
async fn exchange(auth: &mut Authenticated, me: &PeerInfo) -> Result<PairedPeer> {
    // Pair-only admission requires identities on both sides.
    let identity = auth
        .peer_identity
        .take()
        .ok_or(SetupFailure::HandshakeStateMismatch)?;
    let ours = Handshake::PeerInfo {
        name: me.name.clone(),
        os: me.os,
    };
    write(&mut auth.send, &[ours]).await?;
    let Handshake::PeerInfo { name, os } = auth.recv.expect_frame().await? else {
        return Err(unexpected("peer info"));
    };
    if name.chars().count() > MAX_DEVICE_NAME {
        return Err(ProtocolViolation::DeviceNameTooLong.into());
    }
    Ok(PairedPeer { identity, name, os })
}
