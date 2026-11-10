//! LAN sender: capture once, then answer the receiver over pairing and
//! any number of reconnects ([`SendHandle::run`]). The front-end only
//! shows codes and relays the user's answer ([`SendPrompt`]); the pairing
//! policy (warn, ask, attempt cap) lives in `pairing`, the end of a send
//! in `report`, the sender's way in (prepare / resume a send) in
//! `prepare`.

use carrova_crypto::pairing::ResumeSecret;
use carrova_transport::{
    AllowedChunks, Connection, ListenOptions, Listener, ServeEnd, Ticket, TransportError,
};

mod ending;
mod pairing;
mod prepare;
mod report;

use ending::Ending;
use pairing::offer;
pub use pairing::{CodeOffer, SendPrompt};
use report::End;
pub use report::{SendOutcome, SendReport};

use super::session::Session;
use crate::cancel::Cancel;
use crate::capture::Capture;
use crate::error::{CoreError, Result};
use crate::lan::{self, IDLE_POLL, LanMsg, SendAction, SendStep};
use crate::session::progress::PROGRESS_STEP;
use crate::session::{Event, Metered, Phase, ProgressMeter, SessionId};

/// A prepared LAN send, listening for a receiver.
#[derive(Debug)]
pub struct SendHandle {
    s: Session,
    capture: Capture,
    /// Manifest CBOR as persisted (its BLAKE3 is the session's
    /// fingerprint).
    manifest: Vec<u8>,
    allowed: AllowedChunks,
    listener: Listener,
    peer: Peer,
    /// Failed pairing attempts so far (see [`MAX_PAIRING_FAILURES`](crate::lan::MAX_PAIRING_FAILURES)).
    pairing_failures: u32,
    /// Chunk bytes served in this run (progress).
    sent: u64,
    /// Stops [`Self::run`]; see [`crate::cancel`].
    cancel: Cancel,
}

/// Who may connect.
#[derive(Debug)]
enum Peer {
    /// Waiting for a receiver to pair with this ticket's code.
    Offered(Ticket),
    /// Paired: only the receiver holding this secret can (re)connect.
    Paired(ResumeSecret),
}

impl SendHandle {
    fn listen(
        s: Session,
        capture: Capture,
        manifest: Vec<u8>,
        secret: Option<ResumeSecret>,
        cancel: Cancel,
    ) -> Result<Self> {
        let mut listener = Listener::bind(ListenOptions {
            platform: s.engine.platform.clone(),
            cancel: Some(cancel.signal()),
            ..ListenOptions::default()
        })?;
        let peer = match secret {
            Some(secret) => {
                listener.advertise_resume(&secret)?;
                Peer::Paired(secret)
            }
            None => Peer::Offered(offer(&mut listener)?),
        };
        Ok(Self {
            allowed: AllowedChunks::from_manifest(&capture.manifest),
            s,
            capture,
            manifest,
            listener,
            peer,
            pairing_failures: 0,
            sent: 0,
            cancel,
        })
    }

    /// Cancels [`Self::run`] from another thread: take it before calling
    /// `run`. Once paired, the receiver is told and the send stays
    /// resumable ([`Exporter::resume_send`](crate::Exporter::resume_send)); before pairing the session
    /// returns to `Selected`.
    #[must_use]
    pub fn canceller(&self) -> Cancel {
        self.cancel.clone()
    }

    /// Session id.
    #[must_use]
    pub fn id(&self) -> &SessionId {
        &self.s.record.id
    }

    /// Code plus fallback address to show the user while waiting for a
    /// receiver to pair; `None` once paired or for a resumed send (only
    /// the paired receiver can reconnect). Holds the pairing code:
    /// display it, never log it.
    #[must_use]
    pub fn ticket(&self) -> Option<&Ticket> {
        match &self.peer {
            Peer::Offered(t) => Some(t),
            Peer::Paired(_) => None,
        }
    }

    /// UDP port the sender listens on (all IPv4 interfaces).
    #[must_use]
    pub fn port(&self) -> u16 {
        self.listener.port()
    }

    /// Serve the session until the receiver reports it applied (or
    /// aborts): pair, receive the inventory, send the manifest, receive the
    /// plan summary, serve chunks, receive the checklist (state `Done`).
    ///
    /// The code is shown through `prompt` ([`SendPrompt::show_code`]).
    /// After a failed pairing attempt ([`Event::PairingFailed`]) the code
    /// is burned and the user is asked ([`SendPrompt::confirm_new_code`])
    /// before a fresh code is offered; declining ends the send with
    /// [`CoreError::PairingStopped`]. The [`MAX_PAIRING_FAILURES`](crate::lan::MAX_PAIRING_FAILURES)th
    /// failure ends it without asking
    /// ([`CoreError::PairingAttemptsExhausted`]) and leaves the session
    /// `Failed`. Declining, or a cancel before any receiver paired, returns
    /// the session to `Selected` (nothing was sent):
    /// [`Exporter::reopen_after_send`](crate::Exporter::reopen_after_send) then lets the same exporter prepare
    /// a new send with a fresh code.
    ///
    /// A dropped connection ([`Event::PeerLost`]) is re-accepted from the
    /// paired receiver for up to [`RECONNECT_WINDOW`](crate::lan::RECONNECT_WINDOW); after that
    /// [`CoreError::PeerUnreachable`] and the session stays resumable
    /// ([`Exporter::resume_send`](crate::Exporter::resume_send)). The receiver cancelling is
    /// [`CoreError::PeerCancelled`], any other receiver abort but a dry
    /// run [`CoreError::PeerAborted`]. A cancel ([`Self::canceller`]) is
    /// [`CoreError::Cancelled`] (the receiver is told).
    pub fn run(mut self, prompt: &mut dyn SendPrompt) -> Result<SendReport> {
        if let Peer::Offered(ticket) = &self.peer {
            prompt.show_code(ticket, CodeOffer::First);
        }
        let result = self.drive(prompt);
        if let Err(e) = &result {
            match self.ending(e) {
                Ending::Resumable => {}
                Ending::BackToSelection => self.back_to_selection(e),
                Ending::Failed => self.s.fail(e),
            }
        }
        result
    }

    fn drive(&mut self, prompt: &mut dyn SendPrompt) -> Result<SendReport> {
        let mut served = ServeEnd::default();
        loop {
            let (mut conn, mut step) = self.accept(prompt)?;
            match self.converse(&mut conn, &mut step, &mut served) {
                Ok(end) => return self.finish(end, served),
                Err(e) if e.is_connection_lost() => {
                    drop(conn);
                    tracing::info!("connection to the receiver lost; waiting for it");
                    self.s.emit(Event::PeerLost { retrying: true });
                }
                Err(CoreError::Cancelled) => {
                    lan::farewell_cancelled(conn);
                    return Err(CoreError::Cancelled);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Answer the receiver's messages on `conn` ([`SendStep`]) until it
    /// ends the session.
    fn converse(
        &mut self,
        conn: &mut Connection,
        step: &mut SendStep,
        served: &mut ServeEnd,
    ) -> Result<End> {
        loop {
            let msg: LanMsg = match conn.recv(IDLE_POLL) {
                Ok(m) => m,
                Err(TransportError::Timeout) => continue,
                Err(e) => return Err(e.into()),
            };
            let (next, action) = step.on(msg)?;
            *step = next;
            match action {
                SendAction::SendManifest(mut inventory) => {
                    // Only the sanitised name is stored or shown.
                    inventory.device_name = inventory.device_name();
                    self.record_peer_name(&inventory)?;
                    self.s.emit(Event::InventoryReceived(inventory));
                    for part in lan::manifest_messages(&self.manifest) {
                        conn.send(&part)?;
                    }
                }
                SendAction::ShowPlan(summary) => {
                    self.s.emit(Event::PlanSummaryReceived(summary));
                }
                SendAction::Resumed(stage) => {
                    tracing::info!(?stage, "receiver resumed the session");
                }
                SendAction::Serve => {
                    let end = self.serve(conn)?;
                    served.chunks += end.chunks;
                    served.bytes += end.bytes;
                }
                SendAction::Finished(checklist) => return Ok(End::Applied(checklist)),
                SendAction::Aborted(reason) => return Ok(End::Aborted(reason)),
            }
        }
    }

    /// Serve the chunks of one receiver fetch, reporting progress
    /// (`Phase::Send`; `total` is [`Self::allowed_bytes`], an upper bound of
    /// what the receiver fetches). Bytes count across fetches and
    /// reconnects.
    fn serve(&mut self, conn: &mut Connection) -> Result<ServeEnd> {
        let mut meter = ProgressMeter::new(
            self.s.sink(),
            Phase::Send,
            self.allowed_bytes(),
            PROGRESS_STEP,
        )
        .resume(self.sent);
        let source = self.capture.chunk_source(&**self.s.engine.platform());
        let served = conn.serve_chunks(&self.allowed, &mut Metered::new(source, &mut meter));
        self.sent = meter.done();
        Ok(served?)
    }

    /// Bytes of every distinct manifest chunk: the most a receiver can
    /// fetch (it fetches only what its plan needs).
    fn allowed_bytes(&self) -> u64 {
        self.capture
            .manifest
            .unique_chunk_refs()
            .map(|c| u64::from(c.len))
            .sum()
    }
}
