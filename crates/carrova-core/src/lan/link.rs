//! Receiver's connection to the sender, reconnecting automatically.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use carrova_crypto::pairing::ResumeSecret;
use carrova_platform::Platform;
use carrova_transport::{ConnectOptions, Connection, Target, TransportError, connect};

use super::{
    LanMsg, RECONNECT_ATTEMPT, RECONNECT_BACKOFF_MAX, RECONNECT_BACKOFF_MIN, ReceiverStage,
    farewell_cancelled,
};
use crate::cancel::Cancel;
use crate::error::{CoreError, Result};
use crate::session::{Event, EventSink};

/// The receiver's end of a LAN session: the live connection (if any) and
/// the session's resume secret to re-establish it. Every reconnect starts
/// with [`LanMsg::Resume`] naming the receiver's stage.
#[derive(Debug)]
pub(crate) struct Link {
    conn: Option<Connection>,
    secret: ResumeSecret,
    dialer: Box<dyn Dialer>,
    /// Stops reconnecting ([`CoreError::Cancelled`]).
    cancel: Cancel,
}

/// How a [`Link`] reaches the paired sender again.
pub(crate) trait Dialer: Send + fmt::Debug {
    /// One resume attempt (discovery + handshake + resume proofs) within
    /// `timeout`. Any error means no authenticated sender was reached.
    fn resume(
        &mut self,
        secret: &ResumeSecret,
        timeout: Duration,
    ) -> Result<Connection, TransportError>;
}

/// The real transport, on the engine's platform.
pub(crate) struct TransportDialer {
    opts: ConnectOptions,
}

impl TransportDialer {
    /// Dials on `platform`; `cancel` stops a dial and every call on the
    /// connections it makes.
    pub(crate) fn boxed(platform: Arc<dyn Platform>, cancel: &Cancel) -> Box<dyn Dialer> {
        Box::new(Self {
            opts: ConnectOptions {
                platform,
                cancel: Some(cancel.signal()),
            },
        })
    }
}

impl fmt::Debug for TransportDialer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransportDialer").finish_non_exhaustive()
    }
}

impl Dialer for TransportDialer {
    fn resume(
        &mut self,
        secret: &ResumeSecret,
        timeout: Duration,
    ) -> Result<Connection, TransportError> {
        connect(Target::Resume(secret), timeout, &self.opts)
    }
}

impl Link {
    /// A link over a freshly paired connection; reconnects via `dialer`
    /// until `cancel` fires.
    pub(crate) fn paired(conn: Connection, dialer: Box<dyn Dialer>, cancel: Cancel) -> Self {
        let secret = conn.resume_secret().clone();
        Self {
            conn: Some(conn),
            secret,
            dialer,
            cancel,
        }
    }

    /// A link with no connection yet (resume after a restart).
    pub(crate) fn detached(secret: ResumeSecret, dialer: Box<dyn Dialer>, cancel: Cancel) -> Self {
        Self {
            conn: None,
            secret,
            dialer,
            cancel,
        }
    }

    /// End the link after a local cancel: the sender is told if
    /// connected (never reconnects for it).
    pub(crate) fn cancelled(self) {
        if let Some(conn) = self.conn {
            farewell_cancelled(conn);
        }
    }

    /// The live connection; reconnects first (within `window`) if there is
    /// none.
    pub(crate) fn connected(
        &mut self,
        stage: ReceiverStage,
        window: Duration,
        sink: &dyn EventSink,
    ) -> Result<&mut Connection> {
        if self.conn.is_none() {
            self.reconnect(stage, window, sink)?;
        }
        self.conn
            .as_mut()
            .ok_or_else(|| CoreError::Transport(TransportError::PeerLost))
    }

    /// The connection dropped: forget it and tell the front-end that the
    /// engine is reconnecting.
    pub(crate) fn lost(&mut self, sink: &dyn EventSink) {
        if self.conn.take().is_some() {
            tracing::info!("connection to the sender lost; reconnecting");
            sink.emit(Event::PeerLost { retrying: true });
        }
    }

    /// Reconnect with the resume secret, retrying with backoff
    /// ([`RECONNECT_BACKOFF_MIN`] doubling to [`RECONNECT_BACKOFF_MAX`])
    /// until `window` has passed ([`CoreError::PeerUnreachable`]; the
    /// session stays resumable). Then announce `stage`.
    ///
    /// Every dial error is retried: it means no authenticated sender was
    /// reached (a stranger answering discovery, a garbled or failed
    /// handshake), which must never end the session. Only errors on the
    /// authenticated connection afterwards are the sender's own and
    /// returned (a lost connection is retried). A cancel ends the wait
    /// with [`CoreError::Cancelled`].
    pub(crate) fn reconnect(
        &mut self,
        stage: ReceiverStage,
        window: Duration,
        sink: &dyn EventSink,
    ) -> Result<()> {
        self.conn = None;
        let started = Instant::now();
        let mut backoff = RECONNECT_BACKOFF_MIN;
        loop {
            self.cancel.check()?;
            let left = window.saturating_sub(started.elapsed());
            if left.is_zero() {
                return Err(CoreError::PeerUnreachable {
                    waited_secs: window.as_secs(),
                });
            }
            match self
                .dialer
                .resume(&self.secret, RECONNECT_ATTEMPT.min(left))
            {
                Ok(mut conn) => match conn.send(&LanMsg::Resume { stage }) {
                    Ok(()) => {
                        tracing::info!(?stage, "reconnected to the sender");
                        sink.emit(Event::PeerConnected {
                            peer: conn.peer_addr(),
                        });
                        self.conn = Some(conn);
                        return Ok(());
                    }
                    Err(e) if e.is_lost() => {
                        tracing::debug!(error = %e, "reconnected sender dropped at once");
                    }
                    Err(e) => return Err(e.into()),
                },
                Err(TransportError::Cancelled) => return Err(CoreError::Cancelled),
                Err(e) => tracing::debug!(error = %e, "reconnect attempt failed"),
            }
            self.cancel
                .sleep(backoff.min(window.saturating_sub(started.elapsed())))?;
            backoff = (backoff * 2).min(RECONNECT_BACKOFF_MAX);
        }
    }

    /// Send `msg`, reconnecting (within `window`) when the connection is
    /// gone or drops while sending.
    pub(crate) fn send(
        &mut self,
        msg: &LanMsg,
        stage: ReceiverStage,
        window: Duration,
        sink: &dyn EventSink,
    ) -> Result<()> {
        loop {
            match self.connected(stage, window, sink)?.send(msg) {
                Ok(()) => return Ok(()),
                Err(e) if e.is_lost() => self.lost(sink),
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Best-effort final message (checklist or abort) when the session
    /// ended: tries for at most `window`, then closes. Failure is only
    /// logged: the receiver's outcome does not depend on it.
    pub(crate) fn notify(mut self, msg: &LanMsg, window: Duration, sink: &dyn EventSink) {
        if let Err(e) = self.send(msg, ReceiverStage::Transferring, window, sink) {
            tracing::warn!(error = %e, "could not notify the sender");
            sink.emit(Event::Warning {
                message: format!("could not tell the sending machine how the session ended: {e}"),
            });
        }
    }
}

#[cfg(test)]
#[path = "../../tests/lan/link/unit.rs"]
mod tests;
