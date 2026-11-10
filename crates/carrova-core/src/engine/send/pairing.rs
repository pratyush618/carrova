//! The pairing side of a LAN send: what the front-end shows and answers
//! ([`SendPrompt`]), and accepting a receiver — pairing with a code (a
//! fresh one after each failed attempt, at most
//! [`MAX_PAIRING_FAILURES`], and after each
//! [`CODE_LIFETIME`](crate::lan::CODE_LIFETIME)) or the
//! paired receiver reconnecting.

use std::time::{Duration, Instant};

use carrova_crypto::pairing::PairingCode;
use carrova_transport::{Connection, Listener, Ticket, TransportError};
use serde::{Deserialize, Serialize};

use super::{Peer, SendHandle};
use crate::error::{CoreError, Result};
use crate::lan::{IDLE_POLL, Inventory, MAX_PAIRING_FAILURES, RECONNECT_WINDOW, SendStep};
use crate::session::Event;
use crate::store::{self, Source};

/// The front-end's part of a LAN send: display pairing codes and ask the
/// user. Implemented by each front-end (rendering and asking only); when to
/// call it and the attempt cap are the engine's policy.
pub trait SendPrompt {
    /// Show `ticket` (pairing code + fallback address) to the user. Called
    /// once for the first code ([`CodeOffer::First`]) and again for each
    /// code offered after a failed attempt ([`CodeOffer::Renewed`]) or
    /// after the previous one expired ([`CodeOffer::Expired`]). The code
    /// stops working `expires_in` from now. Display it; never log it.
    fn show_code(&mut self, ticket: &Ticket, offer: CodeOffer, expires_in: Duration);

    /// A pairing attempt failed: someone entered a wrong code, which is
    /// now burned. Warn the user loudly that, if it wasn't them, someone
    /// may be guessing, and ask whether to offer a new code. `failures`
    /// attempts failed so far; `remaining` more are allowed (at least 1)
    /// before the send ends ([`MAX_PAIRING_FAILURES`]). `false` (also the
    /// right answer when nobody can be asked) ends the send with
    /// [`CoreError::PairingStopped`].
    fn confirm_new_code(&mut self, failures: u32, remaining: u32) -> bool;
}

/// Which pairing code [`SendPrompt::show_code`] shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CodeOffer {
    /// The send's first code.
    First,
    /// A fresh code after a failed pairing attempt burned the previous one.
    Renewed,
    /// A fresh code after the previous one outlived
    /// [`CODE_LIFETIME`](crate::lan::CODE_LIFETIME).
    Expired,
}

impl SendHandle {
    /// The next authenticated connection: a pairing receiver (a new code
    /// after a failed attempt, see [`Self::pairing_failed`], or once the
    /// code outlived its lifetime) or the paired receiver reconnecting.
    ///
    /// An expired code is replaced only after the wait for it timed out,
    /// which the transport reports only when no attempt holds the code
    /// (one that does is waited for and decides its fate), so a late guess
    /// at the old nameplate is turned away unclaimed and never counts as a
    /// failure.
    pub(super) fn accept(&mut self, prompt: &mut dyn SendPrompt) -> Result<(Connection, SendStep)> {
        loop {
            match &self.peer {
                Peer::Paired(secret) => {
                    self.listener.advertise_resume(secret)?;
                    let conn = match self.listener.accept_resume(secret, RECONNECT_WINDOW) {
                        Ok(c) => c,
                        Err(TransportError::Timeout) => {
                            return Err(CoreError::PeerUnreachable {
                                waited_secs: RECONNECT_WINDOW.as_secs(),
                            });
                        }
                        Err(e) => return Err(e.into()),
                    };
                    self.connected(&conn)?;
                    return Ok((conn, SendStep::AwaitResume));
                }
                Peer::Offered(ticket, at) => {
                    let left = self.code_lifetime.saturating_sub(at.elapsed());
                    if left.is_zero() {
                        tracing::info!("pairing code expired; offering a fresh one");
                        let fresh = offer(&mut self.listener)?;
                        prompt.show_code(&fresh, CodeOffer::Expired, self.code_lifetime);
                        self.peer = Peer::Offered(fresh, Instant::now());
                        continue;
                    }
                    let sink = &self.s;
                    let staked = || sink.emit(Event::PairingInProgress);
                    match self
                        .listener
                        .accept_pairing(&ticket.code, IDLE_POLL.min(left), staked)
                    {
                        Ok(conn) => {
                            let secret = conn.resume_secret().clone();
                            store::write_resume_secret(self.s.platform(), &self.s.dir, &secret)?;
                            // Stop offering the nameplate right away.
                            self.listener.advertise_resume(&secret)?;
                            self.peer = Peer::Paired(secret);
                            tracing::info!(session = %self.s.record.id, "receiver paired");
                            self.connected(&conn)?;
                            return Ok((conn, SendStep::AwaitInventory));
                        }
                        Err(TransportError::PairingFailed) => {
                            let fresh = self.pairing_failed(prompt)?;
                            prompt.show_code(&fresh, CodeOffer::Renewed, self.code_lifetime);
                            self.peer = Peer::Offered(fresh, Instant::now());
                        }
                        Err(TransportError::Timeout) => {}
                        Err(e) => return Err(e.into()),
                    }
                }
            }
        }
    }

    /// A pairing attempt failed (its code is burned): count it, end the
    /// send at [`MAX_PAIRING_FAILURES`], else ask the user and offer a
    /// fresh code only if they agree.
    fn pairing_failed(&mut self, prompt: &mut dyn SendPrompt) -> Result<Ticket> {
        self.pairing_failures = self.pairing_failures.saturating_add(1);
        let failures = self.pairing_failures;
        tracing::warn!(failures, "pairing attempt failed: wrong code");
        self.s.emit(Event::PairingFailed);
        let remaining = MAX_PAIRING_FAILURES.saturating_sub(failures);
        if remaining == 0 {
            return Err(CoreError::PairingAttemptsExhausted { failures });
        }
        if !prompt.confirm_new_code(failures, remaining) {
            tracing::info!("user declined a new pairing code");
            return Err(CoreError::PairingStopped { failures });
        }
        offer(&mut self.listener)
    }

    /// Record the peer's certificate and announce the connection.
    fn connected(&mut self, conn: &Connection) -> Result<()> {
        if let Some(Source::Lan { peer_cert_hash, .. }) = &mut self.s.record.source {
            *peer_cert_hash = Some(conn.peer_cert_hash());
        }
        self.s.save()?;
        self.s.emit(Event::PeerConnected {
            peer: conn.peer_addr(),
        });
        Ok(())
    }

    /// Record the receiver's OS and device name from its `inventory`
    /// (name sanitised; kept when it announces none).
    pub(super) fn record_peer(&mut self, inventory: &Inventory) -> Result<()> {
        self.s.record.peer_os = Some(inventory.os);
        if let (Some(name), Some(Source::Lan { peer_name, .. })) =
            (inventory.device_name(), &mut self.s.record.source)
        {
            *peer_name = Some(name);
        }
        self.s.save()
    }
}

/// Fresh code advertised on `listener`, as a ticket with the fallback
/// address.
pub(super) fn offer(listener: &mut Listener) -> Result<Ticket> {
    let code = PairingCode::generate().map_err(TransportError::from)?;
    listener.advertise_pairing(&code)?;
    Ok(Ticket {
        code,
        addr: listener.fallback_addr(),
    })
}
