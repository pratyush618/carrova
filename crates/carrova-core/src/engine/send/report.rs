//! How a LAN send ends: the receiver's checklist (or a dry run), as a
//! [`SendReport`].

use carrova_transport::ServeEnd;
use serde::{Deserialize, Serialize};

use super::SendHandle;
use crate::checklist::{Checklist, Skipped};
use crate::error::Result;
use crate::key::ItemKey;
use crate::lan::AbortReason;
use crate::session::{Event, State};

/// How the receiver ended a send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SendOutcome {
    /// The receiver applied the plan.
    Applied,
    /// The receiver only did a dry run; nothing was transferred.
    DryRun,
}

/// Outcome of a LAN send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SendReport {
    /// The receiver's end-of-session checklist plus items this machine
    /// could not capture. After a dry run: nothing moved, every item
    /// skipped.
    pub checklist: Checklist,
    /// Chunks served (including re-sends after reconnects).
    pub chunks: u64,
    /// Chunk bytes served.
    pub bytes: u64,
    /// Applied or dry run.
    pub outcome: SendOutcome,
}

/// How one connection's conversation ended.
pub(super) enum End {
    Applied(Checklist),
    Aborted(AbortReason),
}

impl SendHandle {
    /// End the send as the receiver ended it: `Done` with the checklist
    /// (plus items this machine could not capture), or the receiver's
    /// abort as an error.
    pub(super) fn finish(&mut self, end: End, served: ServeEnd) -> Result<SendReport> {
        let (mut checklist, outcome) = match end {
            End::Applied(checklist) => (checklist, SendOutcome::Applied),
            End::Aborted(AbortReason::DryRun) => {
                let reason = AbortReason::DryRun.to_string();
                let skipped = self
                    .capture
                    .manifest
                    .items
                    .iter()
                    .map(|i| Skipped {
                        what: ItemKey::of(i).to_string(),
                        reason: reason.clone(),
                    })
                    .collect();
                let checklist = Checklist {
                    skipped,
                    ..Checklist::default()
                };
                (checklist, SendOutcome::DryRun)
            }
            End::Aborted(reason) => return Err(reason.into()),
        };
        checklist
            .skipped
            .extend(self.capture.failed.iter().cloned());
        self.s.record.checklist = Some(checklist.clone());
        self.s.record.bytes_moved = Some(served.bytes);
        self.s.transition(State::Done)?;
        if outcome == SendOutcome::Applied
            && let Some(peer) = self.known_peer.take()
        {
            self.s.remember_peer(&peer);
        }
        self.s.emit(Event::Finished {
            checklist: checklist.clone(),
        });
        Ok(SendReport {
            checklist,
            chunks: served.chunks,
            bytes: served.bytes,
            outcome,
        })
    }
}
