//! How a failed LAN send leaves its session: resumable, back at the
//! selection (nobody paired), or failed.

use super::{Peer, SendHandle};
use crate::error::CoreError;
use crate::session::State;

/// What a failed [`SendHandle::run`] leaves of the session.
pub(super) enum Ending {
    /// Kept as is: the paired receiver can reconnect (resume).
    Resumable,
    /// Nobody paired: back to the selection.
    BackToSelection,
    /// The session fails.
    Failed,
}

impl SendHandle {
    /// What `e` leaves of the send. An unreachable or cancelling receiver,
    /// or the user cancelling, leave a paired send resumable. Before
    /// pairing nothing left this machine: the user cancelling or declining
    /// a new code returns the session to its selection (a new send offers
    /// a fresh code); the attempt cap and any other failure end it.
    pub(super) fn ending(&self, e: &CoreError) -> Ending {
        let paired = matches!(self.peer, Peer::Paired(_));
        match e {
            CoreError::PeerUnreachable { .. } => Ending::Resumable,
            CoreError::Cancelled | CoreError::PeerCancelled if paired => Ending::Resumable,
            CoreError::Cancelled | CoreError::PairingStopped { .. } if !paired => {
                Ending::BackToSelection
            }
            _ => Ending::Failed,
        }
    }

    /// Back to `Selected` after an unpaired send (`Failed` with `e` if
    /// that cannot be recorded).
    pub(super) fn back_to_selection(&mut self, e: &CoreError) {
        let source = self.s.record.source.take();
        if let Err(err) = self.s.transition(State::Selected) {
            tracing::warn!(error = %err, "could not return the send to its selection");
            self.s.record.source = source;
            self.s.fail(e);
        }
    }
}
