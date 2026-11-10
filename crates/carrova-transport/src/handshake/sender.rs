//! Sender side: admit an incoming connection, then prove the secret.

use std::sync::Arc;

use carrova_crypto::pairing::{Pake, PakeMessage, PakeRole};
use quinn::SendStream;
use tokio::time::{Instant, timeout, timeout_at};

use super::io::{read_confirm, unexpected, write};
use super::keys::Keys;
use super::{Authenticated, Expected, binding};
use crate::PROTOCOL_VERSION;
use crate::error::{Result, SetupFailure, TransportError};
use crate::tls::Identity;
use crate::tuning::{HANDSHAKE_TIMEOUT, PRE_PAKE_TIMEOUT};
use crate::wire::{CloseCode, Handshake, RejectReason, StreamReader};

/// How one incoming connection ended without authenticating.
#[derive(Debug)]
pub(crate) enum AttemptFailure {
    /// Nothing secret was put at stake (wrong nameplate or tag, replayed
    /// `Hello`, garbage, pre-PAKE timeout, code already claimed): keep
    /// waiting for the right receiver.
    Retry(TransportError),
    /// The secret was put at stake (PAKE message received, or resume proof
    /// exchanged) and the attempt failed.
    Failed(TransportError),
}

/// Sender: run the handshake on one admitted incoming connection, closed
/// with the matching code unless it authenticates. The receiver must put
/// the secret at stake within [`PRE_PAKE_TIMEOUT`] (and before
/// `deadline`); from then on the attempt gets [`HANDSHAKE_TIMEOUT`], even
/// past `deadline`, so its own result decides the code's fate.
pub(crate) async fn sender_attempt(
    incoming: quinn::Incoming,
    identity: Arc<Identity>,
    expected: Expected,
    deadline: Instant,
) -> Result<Authenticated, AttemptFailure> {
    use AttemptFailure::Retry;

    let pre_pake = deadline.min(Instant::now() + PRE_PAKE_TIMEOUT);
    let conn = timeout_at(pre_pake, incoming)
        .await
        .map_err(|_| Retry(TransportError::Timeout))?
        .map_err(|e| Retry(TransportError::connection(&e)))?;
    let staked = timeout_at(pre_pake, stake(&conn, &expected))
        .await
        .unwrap_or(Err(Retry(TransportError::Timeout)));
    let outcome = match staked {
        Ok(staked) => timeout(
            HANDSHAKE_TIMEOUT,
            prove(&conn, &identity, &expected, staked),
        )
        .await
        .unwrap_or(Err(TransportError::Timeout))
        .map_err(AttemptFailure::Failed),
        Err(e) => Err(e),
    };
    match &outcome {
        Ok(_) => {}
        Err(Retry(_)) => CloseCode::Rejected.close(&conn),
        Err(AttemptFailure::Failed(_)) => CloseCode::PairingFailed.close(&conn),
    }
    outcome
}

/// What the receiver put at stake: its control stream and, for pairing,
/// its PAKE message.
struct Staked {
    send: SendStream,
    recv: StreamReader,
    peer_pake: Option<PakeMessage>,
}

/// Sender, before anything secret is used: read `Hello`, check the
/// intent, and (pairing) take the code with the receiver's PAKE message.
async fn stake(conn: &quinn::Connection, expected: &Expected) -> Result<Staked, AttemptFailure> {
    use AttemptFailure::Retry;

    let (mut send, recv) = conn
        .accept_bi()
        .await
        .map_err(|e| Retry(TransportError::connection(&e)))?;
    let mut recv = StreamReader::new(recv);
    let Handshake::Hello { proto, intent } = recv.expect_frame().await.map_err(Retry)? else {
        return Err(Retry(unexpected("hello")));
    };
    let reject = if proto != PROTOCOL_VERSION {
        Some(RejectReason::Version)
    } else if !expected.admit(&intent) {
        Some(RejectReason::NotOffered)
    } else {
        None
    };
    if let Some(reason) = reject {
        // Best effort: the `Rejected` close tells the receiver anyway.
        let _ = write(&mut send, &[Handshake::Reject(reason)]).await;
        return Err(Retry(TransportError::NotFound));
    }
    write(&mut send, &[Handshake::Welcome])
        .await
        .map_err(Retry)?;
    let peer_pake = match expected {
        Expected::Pair { claim, .. } => {
            let Handshake::Pake(msg) = recv.expect_frame().await.map_err(Retry)? else {
                return Err(Retry(unexpected("PAKE message")));
            };
            // One guess per code: the first PAKE message claims it.
            if !claim.take() {
                return Err(Retry(TransportError::NotFound));
            }
            Some(PakeMessage::from_bytes(msg))
        }
        Expected::Resume { .. } => None,
    };
    Ok(Staked {
        send,
        recv,
        peer_pake,
    })
}

/// Sender, with the secret at stake: run the PAKE (pairing) and both
/// confirmations.
async fn prove(
    conn: &quinn::Connection,
    identity: &Identity,
    expected: &Expected,
    staked: Staked,
) -> Result<Authenticated> {
    let Staked {
        mut send,
        mut recv,
        peer_pake,
    } = staked;
    let (binding, peer_cert) = binding(conn, identity, PakeRole::Sender)?;
    let keys = match (expected, peer_pake) {
        (Expected::Pair { code, .. }, Some(peer_msg)) => {
            let (pake, own_msg) = Pake::start(code, PakeRole::Sender);
            let keys = Keys::Pake(
                pake.finish(&peer_msg)
                    .map_err(|_| TransportError::PairingFailed)?,
            );
            let confirm = keys.confirm(PakeRole::Sender, &binding)?;
            write(
                &mut send,
                &[
                    Handshake::Pake(own_msg.into_bytes()),
                    Handshake::Confirm(confirm.0),
                ],
            )
            .await?;
            keys
        }
        (Expected::Resume { secret: resume, .. }, None) => {
            let keys = Keys::Resume(resume);
            let confirm = keys.confirm(PakeRole::Sender, &binding)?;
            write(&mut send, &[Handshake::Confirm(confirm.0)]).await?;
            keys
        }
        _ => return Err(SetupFailure::HandshakeStateMismatch.into()),
    };
    let theirs = read_confirm(&mut recv).await?;
    keys.verify(PakeRole::Receiver, &binding, &theirs)?;
    write(&mut send, &[Handshake::Established]).await?;
    Ok(Authenticated {
        conn: conn.clone(),
        send,
        recv,
        peer_cert,
        resume: keys.resume_secret(),
    })
}
