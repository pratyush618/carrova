//! Sender accept loop: admit incoming connections into a bounded set of
//! concurrent handshakes until one authenticates.

use std::sync::Arc;

use tokio::task::{JoinError, JoinSet};
use tokio::time::{Instant, sleep_until};

use crate::admission::{Admission, HandshakeSlots};
use crate::error::{Result, SetupFailure, TransportError};
use crate::handshake::{AttemptFailure, Authenticated, Expected, sender_attempt};
use crate::tls::Identity;
use crate::tuning::{HANDSHAKE_TIMEOUT, MAX_PENDING_HANDSHAKES};

/// Sender: accept connections on `endpoint` until one authenticates for
/// `expected` or `deadline` passes. Up to [`MAX_PENDING_HANDSHAKES`] run
/// concurrently (see [`crate::admission`]); each must put the secret at
/// stake quickly, so stalled connections cannot block a legitimate
/// receiver.
///
/// Pairing: one guess per code. The attempt that claimed the code decides
/// its fate: success returns the connection, failure
/// [`TransportError::PairingFailed`]. If the deadline passes while it
/// runs, the loop waits for it (at most [`HANDSHAKE_TIMEOUT`]) and
/// refuses newcomers meanwhile.
/// `on_staked` runs once, when an attempt puts the secret at stake
/// (pairing: the receiver's PAKE message claimed the code).
/// Resume: failed proofs are logged and the sender keeps waiting (the tag
/// is public, so a stranger must not be able to end the wait).
pub(crate) async fn accept(
    endpoint: &quinn::Endpoint,
    identity: &Arc<Identity>,
    expected: &Expected,
    deadline: Instant,
    on_staked: &mut dyn FnMut(),
) -> Result<Authenticated> {
    let mut announced = false;
    let slots = HandshakeSlots::new(MAX_PENDING_HANDSHAKES);
    let mut attempts = JoinSet::new();
    // Set once the deadline passed with the secret at stake.
    let mut overtime: Option<Instant> = None;
    loop {
        tokio::select! {
            incoming = endpoint.accept() => {
                let incoming =
                    incoming.ok_or_else(|| TransportError::from(SetupFailure::EndpointClosed))?;
                if overtime.is_some() {
                    incoming.refuse();
                    continue;
                }
                match slots.admit(&incoming) {
                    Admission::Validate => validate(incoming),
                    Admission::Refuse(why) => {
                        tracing::debug!(?why, "incoming connection refused");
                        incoming.refuse();
                    }
                    Admission::Admit(slot) => {
                        let attempt =
                            sender_attempt(incoming, Arc::clone(identity), expected.clone(), deadline);
                        attempts.spawn(async move {
                            let _slot = slot;
                            attempt.await
                        });
                    }
                }
            }
            () = expected.staked(), if !announced => {
                announced = true;
                on_staked();
            }
            Some(joined) = attempts.join_next() => {
                if let Some(end) = settle(joined, expected) {
                    return end;
                }
            }
            () = sleep_until(deadline), if overtime.is_none() => {
                if !expected.at_stake() {
                    return Err(TransportError::Timeout);
                }
                overtime = Some(Instant::now() + HANDSHAKE_TIMEOUT);
            }
            () = sleep_until(overtime.unwrap_or(deadline)), if overtime.is_some() => {
                // The attempt holding the code never reported back.
                return Err(TransportError::PairingFailed);
            }
        }
    }
}

/// Ask an unvalidated source to prove its address (QUIC Retry); a source
/// that already got one and still is not validated is ignored.
fn validate(incoming: quinn::Incoming) {
    if incoming.may_retry() {
        if let Err(e) = incoming.retry() {
            e.into_incoming().ignore();
        }
    } else {
        incoming.ignore();
    }
}

/// How one finished attempt ends the accept loop, if it does.
fn settle(
    joined: std::result::Result<std::result::Result<Authenticated, AttemptFailure>, JoinError>,
    expected: &Expected,
) -> Option<Result<Authenticated>> {
    match joined {
        Ok(Ok(auth)) => Some(Ok(auth)),
        Ok(Err(AttemptFailure::Retry(e))) => {
            tracing::debug!(error = %e, "handshake attempt turned away");
            None
        }
        Ok(Err(AttemptFailure::Failed(e))) => {
            tracing::warn!(error = %e, "authentication failed");
            matches!(expected, Expected::Pair { .. }).then_some(Err(TransportError::PairingFailed))
        }
        Err(e) => {
            tracing::debug!(error = %e, "handshake task ended");
            None
        }
    }
}
