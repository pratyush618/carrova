//! Receiver: dial the sender's candidate addresses and race full
//! handshakes, within a per-call budget of code guesses and candidates.

use std::collections::HashSet;
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use carrova_crypto::pairing::CertHash;
use carrova_platform::Platform;
use quinn::{ConnectionError, TransportErrorCode};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::{Instant, sleep_until, timeout_at};

use crate::discovery::discover;
use crate::error::{Result, TransportError};
use crate::handshake::{self, Authenticated, Secret};
use crate::tls::{self, Identity};
use crate::tuning::{
    BUSY_RETRY, MAX_PAKE_GUESSES_PER_CONNECT, MAX_RACED_CANDIDATES, RACE_SETTLE, TLS_SERVER_NAME,
};
use crate::wire::CloseCode;

/// Code guesses (PAKE messages) left to one connect call, shared by all of
/// its candidates.
#[derive(Debug)]
pub(crate) struct PakeBudget(AtomicUsize);

impl PakeBudget {
    /// `guesses` PAKE messages may be sent.
    pub(crate) fn new(guesses: usize) -> Self {
        Self(AtomicUsize::new(guesses))
    }

    /// Spend one guess: `false` (and nothing spent) once none are left.
    pub(crate) fn spend(&self) -> bool {
        self.0
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
            .is_ok()
    }

    /// No guesses left.
    pub(crate) fn exhausted(&self) -> bool {
        self.0.load(Ordering::Acquire) == 0
    }
}

/// Distinct candidate addresses admitted to one race, at most
/// [`MAX_RACED_CANDIDATES`].
#[derive(Debug, Default)]
struct Candidates(HashSet<SocketAddrV4>);

impl Candidates {
    /// Admit `addr` unless already seen or the race is full.
    fn admit(&mut self, addr: SocketAddrV4) -> bool {
        !self.is_full() && self.0.insert(addr)
    }

    fn is_full(&self) -> bool {
        self.0.len() >= MAX_RACED_CANDIDATES
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Senders (by TLS certificate) one race has started a handshake with.
///
/// A sender reachable at several addresses (loopback and LAN, a VM
/// bridge, …) presents the same certificate on each, and TLS 1.3 proves
/// possession of its key before any claim. Letting only the first
/// connection per certificate run the handshake means at most one
/// connection to a sender can authenticate per connect call, so sender and
/// receiver can never settle on different connections.
#[derive(Debug, Default)]
struct SenderClaims(Mutex<HashSet<CertHash>>);

impl SenderClaims {
    /// Claim the sender presenting `cert`: `true` for its first connection
    /// only.
    fn claim(&self, cert: CertHash) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(cert)
    }
}

/// Everything one receiver handshake attempt needs (cheap to clone).
#[derive(Clone)]
pub(crate) struct Dialer {
    endpoint: quinn::Endpoint,
    config: quinn::ClientConfig,
    identity: Arc<Identity>,
    secret: Secret,
    budget: Arc<PakeBudget>,
    deadline: Instant,
}

impl Dialer {
    /// One connect call's dialer: a fresh [`PakeBudget`] of
    /// [`MAX_PAKE_GUESSES_PER_CONNECT`].
    pub(crate) fn new(
        endpoint: quinn::Endpoint,
        config: quinn::ClientConfig,
        identity: Arc<Identity>,
        secret: Secret,
        deadline: Instant,
    ) -> Self {
        Self {
            endpoint,
            config,
            identity,
            secret,
            budget: Arc::new(PakeBudget::new(MAX_PAKE_GUESSES_PER_CONNECT)),
            deadline,
        }
    }

    /// Dial `addr` and run the full handshake.
    pub(crate) async fn authenticate(self, addr: SocketAddrV4) -> Result<Authenticated> {
        let conn = self.dial(addr).await?;
        self.handshake(conn).await
    }

    /// Dial `addr` and run the full handshake unless another candidate
    /// already claimed the sender behind it (`Ok(None)`: that connection
    /// is closed before anything is said on it).
    async fn authenticate_once(
        self,
        addr: SocketAddrV4,
        senders: Arc<SenderClaims>,
    ) -> Result<Option<Authenticated>> {
        let conn = self.dial(addr).await?;
        let cert = CertHash::of(&tls::peer_cert(&conn)?);
        if !senders.claim(cert) {
            tracing::debug!(%addr, "sender already reached at another address");
            CloseCode::Normal.close(&conn);
            return Ok(None);
        }
        self.handshake(conn).await.map(Some)
    }

    /// Authenticate the established `conn` with this dialer's secret.
    async fn handshake(&self, conn: quinn::Connection) -> Result<Authenticated> {
        handshake::connect(
            conn,
            &self.identity,
            &self.secret,
            &self.budget,
            self.deadline,
        )
        .await
    }

    /// Connect to `addr`, redialling every [`BUSY_RETRY`] while the sender
    /// refuses us for lack of a free handshake slot.
    async fn dial(&self, addr: SocketAddrV4) -> Result<quinn::Connection> {
        loop {
            let connecting = self
                .endpoint
                .connect_with(self.config.clone(), SocketAddr::V4(addr), TLS_SERVER_NAME)
                .map_err(|e| TransportError::internal("connect", e))?;
            match timeout_at(self.deadline, connecting).await {
                Err(_) => return Err(TransportError::Timeout),
                Ok(Ok(conn)) => return Ok(conn),
                Ok(Err(e)) if refused(&e) => {
                    tracing::debug!(%addr, "sender busy; redialling");
                    let retry = Instant::now() + BUSY_RETRY;
                    if retry >= self.deadline {
                        return Err(TransportError::Timeout);
                    }
                    sleep_until(retry).await;
                }
                Ok(Err(e)) => return Err(TransportError::connection(&e)),
            }
        }
    }
}

/// The sender refused the connection (its handshake slots were busy).
fn refused(e: &ConnectionError) -> bool {
    matches!(e, ConnectionError::ConnectionClosed(close)
        if close.error_code == TransportErrorCode::CONNECTION_REFUSED)
}

/// Discover the sender and run the full handshake with every candidate
/// address in parallel (a host may advertise several, e.g. a VM bridge,
/// and anyone on the LAN can answer discovery). Only the first connection
/// to each sender (by certificate, see [`SenderClaims`]) runs the
/// handshake. The first candidate that authenticates wins; failed
/// candidates are dropped. At most [`MAX_RACED_CANDIDATES`] are dialled
/// and [`MAX_PAKE_GUESSES_PER_CONNECT`] code guesses spent; once those are
/// used up and no attempt is left, the race ends. Once every candidate so
/// far failed authentication, [`RACE_SETTLE`] more is allowed for another
/// before reporting [`TransportError::PairingFailed`].
///
/// Known limit: if the claimed connection dies after TLS but before
/// authenticating, this call does not fall back to the sender's other
/// addresses; it waits for its deadline and returns a retryable error
/// (never [`TransportError::PairingFailed`]). Callers retry.
pub(crate) async fn race(dialer: Dialer, platform: &Arc<dyn Platform>) -> Result<Authenticated> {
    let found = discover(platform, dialer.secret.subject(), dialer.deadline)?;
    race_found(dialer, found).await
}

/// [`race`] over the candidate addresses `found` yields.
async fn race_found(
    dialer: Dialer,
    mut found: mpsc::Receiver<SocketAddrV4>,
) -> Result<Authenticated> {
    let deadline = dialer.deadline;
    let mut candidates = Candidates::default();
    let senders = Arc::new(SenderClaims::default());
    let mut attempts = JoinSet::new();
    let mut discovering = true;
    let mut auth_failed = false;
    let mut settle: Option<Instant> = None;
    let mut last_error = None;
    loop {
        tokio::select! {
            next = found.recv(), if discovering => match next {
                Some(addr) => {
                    if !dialer.budget.exhausted() && candidates.admit(addr) {
                        tracing::debug!(%addr, "sender candidate found");
                        attempts.spawn(dialer.clone().authenticate_once(addr, Arc::clone(&senders)));
                        settle = None;
                    }
                }
                None => discovering = false,
            },
            Some(joined) = attempts.join_next() => {
                match joined {
                    Ok(Ok(Some(auth))) => return Ok(auth),
                    Ok(Ok(None)) => {}
                    Ok(Err(TransportError::PairingFailed)) => auth_failed = true,
                    Ok(Err(e)) => last_error = Some(e),
                    Err(e) => last_error = Some(TransportError::internal("handshake task", e)),
                }
                if attempts.is_empty() {
                    if dialer.budget.exhausted() || candidates.is_full() {
                        break;
                    }
                    if auth_failed {
                        settle = Some(deadline.min(Instant::now() + RACE_SETTLE));
                    }
                }
            },
            () = sleep_until(settle.unwrap_or(deadline)), if settle.is_some() => {
                return Err(TransportError::PairingFailed);
            }
            () = sleep_until(deadline) => break,
        }
    }
    Err(if auth_failed {
        TransportError::PairingFailed
    } else if candidates.is_empty() {
        TransportError::NotFound
    } else {
        last_error.unwrap_or(TransportError::Timeout)
    })
}

#[cfg(test)]
#[path = "../tests/race/unit.rs"]
mod tests;
