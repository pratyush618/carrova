//! Receiver side: open the control stream and authenticate with a
//! [`Secret`].

use carrova_crypto::pairing::{Pake, PakeMessage, PakeRole};
use quinn::SendStream;
use tokio::time::{Instant, timeout_at};

use super::io::{read_confirm, unexpected, write};
use super::keys::Keys;
use super::{Authenticated, Secret, binding};
use crate::PROTOCOL_VERSION;
use crate::error::{ProtocolViolation, Result, TransportError};
use crate::race::PakeBudget;
use crate::tls::Identity;
use crate::tuning::HANDSHAKE_TIMEOUT;
use crate::wire::{CloseCode, Handshake, RejectReason, StreamReader};

/// Receiver: authenticate an established QUIC connection with `secret`.
/// On failure the connection is closed with the matching code.
pub(crate) async fn connect(
    conn: quinn::Connection,
    identity: &Identity,
    secret: &Secret,
    budget: &PakeBudget,
    deadline: Instant,
) -> Result<Authenticated> {
    let deadline = deadline.min(Instant::now() + HANDSHAKE_TIMEOUT);
    let result = timeout_at(deadline, receiver_attempt(&conn, identity, secret, budget))
        .await
        .unwrap_or(Err(TransportError::Timeout));
    if let Err(e) = &result {
        let code = match e {
            TransportError::PairingFailed => CloseCode::PairingFailed,
            e if e.is_violation() => CloseCode::Protocol,
            _ => CloseCode::Normal,
        };
        code.close(&conn);
    }
    result
}

async fn receiver_attempt(
    conn: &quinn::Connection,
    identity: &Identity,
    secret: &Secret,
    budget: &PakeBudget,
) -> Result<Authenticated> {
    let (mut send, mut recv) = open_control(conn, secret).await?;
    let keys = session_keys(&mut send, &mut recv, secret, budget).await?;
    confirm(conn, identity, send, recv, keys).await
}

/// Maps a failure to `NotFound` when the sender closed with `Rejected`.
fn rejected(conn: &quinn::Connection) -> impl Fn(TransportError) -> TransportError + Copy + '_ {
    // A sender that rejects us closes with `Rejected`: that means "not
    // offered here", whichever message we were waiting for.
    move |e: TransportError| match conn.close_reason() {
        Some(quinn::ConnectionError::ApplicationClosed(c))
            if CloseCode::from_wire(c.error_code) == Some(CloseCode::Rejected) =>
        {
            TransportError::NotFound
        }
        _ => e,
    }
}

/// Open the control stream, send `Hello`, and wait for `Welcome`.
async fn open_control(
    conn: &quinn::Connection,
    secret: &Secret,
) -> Result<(SendStream, StreamReader)> {
    let rejected = rejected(conn);
    let (mut send, recv) = conn
        .open_bi()
        .await
        .map_err(|e| TransportError::connection(&e))?;
    let mut recv = StreamReader::new(recv);
    let hello = Handshake::Hello {
        proto: PROTOCOL_VERSION,
        intent: secret.intent()?,
    };
    write(&mut send, &[hello]).await.map_err(rejected)?;
    match recv.expect_frame().await.map_err(rejected)? {
        Handshake::Welcome => {}
        Handshake::Reject(RejectReason::NotOffered) => return Err(TransportError::NotFound),
        Handshake::Reject(RejectReason::Version) => {
            return Err(ProtocolViolation::IncompatibleVersion.into());
        }
        _ => return Err(unexpected("welcome")),
    }
    Ok((send, recv))
}

/// Derive the confirmation keys: run the PAKE (pairing) or use the resume
/// secret.
async fn session_keys<'a>(
    send: &mut SendStream,
    recv: &mut StreamReader,
    secret: &'a Secret,
    budget: &PakeBudget,
) -> Result<Keys<'a>> {
    Ok(match secret {
        Secret::Pair(code) => {
            // Every PAKE message is a guess at the code for whoever
            // answers: out of guesses, stop before sending one.
            if !budget.spend() {
                return Err(TransportError::PairingFailed);
            }
            let (pake, msg) = Pake::start(code, PakeRole::Receiver);
            write(send, &[Handshake::Pake(msg.into_bytes())]).await?;
            let Handshake::Pake(peer_msg) = recv.expect_frame().await? else {
                return Err(unexpected("PAKE message"));
            };
            Keys::Pake(
                pake.finish(&PakeMessage::from_bytes(peer_msg))
                    .map_err(|_| TransportError::PairingFailed)?,
            )
        }
        Secret::Resume(resume) => Keys::Resume(resume),
    })
}

/// Exchange confirmations (the sender's first) and wait for `Established`.
async fn confirm(
    conn: &quinn::Connection,
    identity: &Identity,
    mut send: SendStream,
    mut recv: StreamReader,
    keys: Keys<'_>,
) -> Result<Authenticated> {
    let (binding, peer_cert) = binding(conn, identity, PakeRole::Receiver)?;
    let theirs = read_confirm(&mut recv).await?;
    keys.verify(PakeRole::Sender, &binding, &theirs)?;
    let ours = keys.confirm(PakeRole::Receiver, &binding)?;
    write(&mut send, &[Handshake::Confirm(ours.0)]).await?;
    match recv.expect_frame().await? {
        Handshake::Established => Ok(Authenticated {
            conn: conn.clone(),
            send,
            recv,
            peer_cert,
            resume: keys.resume_secret(),
        }),
        _ => Err(unexpected("established")),
    }
}
