//! The desktop side of the engine's pairing prompts ([`SendPrompt`]).
//!
//! Codes are emitted as [`BridgeEvent::PairingCode`] (the UI shows them;
//! they are never logged). After a wrong code the engine asks whether to
//! offer a new one: the worker emits [`BridgeEvent::ConfirmNewCode`] and
//! blocks until the `answer_pairing` command answers, the session is
//! cancelled, or [`ANSWER_TIMEOUT`] passes. No answer is a "no", the safe
//! choice when nobody is at the screen.
//!
//! Linking a device asks the same way: the found device is emitted as
//! [`BridgeEvent::PairPeerFound`] and [`PeerAnswer`] waits for the
//! `answer_pair_peer` command.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use carrova_core::{Cancel, CodeOffer, SendPrompt, SessionId, Ticket};
use carrova_view::ecosystem::PairPeerView;
use carrova_view::ipc::BridgeEvent;
use carrova_view::lan::code_view;

use crate::events::Emit;

/// How long a "show a new code?" question waits for the user.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Open "show a new code?" questions, by sending session.
#[derive(Debug, Default)]
pub struct PairingAnswers(Mutex<HashMap<SessionId, SyncSender<bool>>>);

/// No question is waiting for this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoQuestion;

impl PairingAnswers {
    /// Open a question for `session` (replacing a stale one).
    fn open(&self, session: &SessionId) -> Receiver<bool> {
        let (answer, question) = mpsc::sync_channel(1);
        self.lock().insert(session.clone(), answer);
        question
    }

    /// Close `session`'s question (answered or timed out).
    fn close(&self, session: &SessionId) {
        self.lock().remove(session);
    }

    /// Answer `session`'s open question.
    ///
    /// # Errors
    ///
    /// [`NoQuestion`] if none is open.
    pub fn answer(&self, session: &SessionId, offer_new_code: bool) -> Result<(), NoQuestion> {
        let answer = self.lock().remove(session).ok_or(NoQuestion)?;
        answer.try_send(offer_new_code).map_err(|_| NoQuestion)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, SyncSender<bool>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// How often an open "is this your device?" question checks for a cancel.
const CANCEL_POLL: Duration = Duration::from_millis(100);

/// The open "is this your device?" question of the pairing in progress
/// (the connect slot runs one at a time).
#[derive(Debug, Default)]
pub struct PeerAnswer(Mutex<Option<SyncSender<bool>>>);

impl PeerAnswer {
    /// Show `peer` on `out` and wait up to `within` for the user's
    /// answer. No answer, or `cancel` firing, is a no.
    pub fn ask(
        &self,
        out: &dyn Emit,
        peer: PairPeerView,
        cancel: &Cancel,
        within: Duration,
    ) -> bool {
        let (answer, question) = mpsc::sync_channel(1);
        *self.lock() = Some(answer);
        out.emit(&BridgeEvent::PairPeerFound { peer });
        let deadline = Instant::now() + within;
        let yes = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match question.recv_timeout(left.min(CANCEL_POLL)) {
                Ok(yes) => break yes,
                Err(RecvTimeoutError::Timeout) if !left.is_zero() && !cancel.is_cancelled() => {}
                Err(_) => break false,
            }
        };
        *self.lock() = None;
        yes
    }

    /// Answer the open question.
    ///
    /// # Errors
    ///
    /// [`NoQuestion`] if none is open.
    pub fn answer(&self, accept: bool) -> Result<(), NoQuestion> {
        let answer = self.lock().take().ok_or(NoQuestion)?;
        answer.try_send(accept).map_err(|_| NoQuestion)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<SyncSender<bool>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// [`SendPrompt`] of one sending session, used on its worker thread.
pub struct BridgePrompt {
    session: SessionId,
    out: Arc<dyn Emit>,
    answers: Arc<PairingAnswers>,
    timeout: Duration,
}

impl std::fmt::Debug for BridgePrompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgePrompt")
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

impl BridgePrompt {
    /// Prompts of `session`, emitted on `out`, answered through `answers`
    /// within `timeout`.
    pub fn new(
        session: SessionId,
        out: Arc<dyn Emit>,
        answers: Arc<PairingAnswers>,
        timeout: Duration,
    ) -> Self {
        Self {
            session,
            out,
            answers,
            timeout,
        }
    }
}

impl SendPrompt for BridgePrompt {
    fn show_code(&mut self, ticket: &Ticket, offer: CodeOffer, expires_in: Duration) {
        self.out.emit(&BridgeEvent::PairingCode {
            session: self.session.clone(),
            code: code_view(ticket, offer, expires_in),
        });
    }

    fn confirm_new_code(&mut self, failures: u32, remaining: u32) -> bool {
        let question = self.answers.open(&self.session);
        self.out.emit(&BridgeEvent::ConfirmNewCode {
            session: self.session.clone(),
            failures,
            remaining,
        });
        let answer = question.recv_timeout(self.timeout).unwrap_or(false);
        self.answers.close(&self.session);
        answer
    }
}

#[cfg(test)]
#[path = "../tests/pairing/unit.rs"]
mod tests;
