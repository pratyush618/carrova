//! Phone notifications and the `Notify` lane.
//!
//! Wire (opener = sender), via [`request`]/[`serve`]: opener → `NotifyFrame`
//! (a message from the phone or an action from the laptop); the receiver
//! answers with an [`ActionResult`] (`Done` acknowledges a message) and
//! finishes. Over-limit frames are refused unanswered.

use std::fmt;
use std::time::Duration;

use carrova_platform::phone::Note;
use carrova_transport::{Connection, Stream};
use serde::{Deserialize, Serialize};

use crate::LinkError;
use crate::lane::LaneKind;
use crate::reply::{Responder, request, serve};

/// Longest notification text, in bytes.
pub const MAX_NOTE_TEXT: usize = 4096;
/// Longest notification title, in bytes.
pub const MAX_NOTE_TITLE: usize = 256;
/// Longest inline reply, in bytes.
pub const MAX_REPLY: usize = 4096;
/// Largest app icon, in bytes.
pub const MAX_ICON: usize = 65536;
/// Most notifications in one snapshot.
pub const MAX_SNAPSHOT: usize = 50;

/// How long the sender waits for the receiver's answer.
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(10);

/// What the phone tells the laptop.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifyMsg {
    /// A notification appeared or changed.
    Posted(Note),
    /// A notification went away.
    Removed {
        /// Its key.
        key: String,
    },
    /// Everything currently showing, at most [`MAX_SNAPSHOT`].
    Snapshot(Vec<Note>),
}

/// What the laptop asks the phone to do with a notification.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotifyAction {
    /// Send an inline reply.
    Reply {
        /// Notification key.
        key: String,
        /// Reply text, at most [`MAX_REPLY`] bytes.
        text: String,
    },
    /// Dismiss it on the phone.
    Dismiss {
        /// Notification key.
        key: String,
    },
    /// Open it on the phone.
    Open {
        /// Notification key.
        key: String,
    },
}

/// How the phone got on with an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionResult {
    /// Done (also the acknowledgement of a [`NotifyMsg`]).
    Done,
    /// The notification no longer exists.
    Gone,
    /// It could not be done.
    Failed,
}

/// First frame of a `Notify` lane: both directions share one lane kind.
#[derive(Serialize, Deserialize)]
enum NotifyFrame {
    Msg(NotifyMsg),
    Action(NotifyAction),
}

/// What arrived on an accepted `Notify` lane.
#[derive(Debug)]
pub enum Incoming {
    /// A message, already acknowledged.
    Msg(NotifyMsg),
    /// An action to carry out, then answer through the [`Responder`].
    Action(NotifyAction, Responder<ActionResult>),
}

// Debug never prints titles, text or reply bodies.
impl fmt::Debug for NotifyMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Posted(_) => "NotifyMsg::Posted(..)",
            Self::Removed { .. } => "NotifyMsg::Removed(..)",
            Self::Snapshot(_) => "NotifyMsg::Snapshot(..)",
        })
    }
}

impl fmt::Debug for NotifyAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Reply { .. } => "NotifyAction::Reply(..)",
            Self::Dismiss { .. } => "NotifyAction::Dismiss(..)",
            Self::Open { .. } => "NotifyAction::Open(..)",
        })
    }
}

/// Cut `note` to the wire limits: text and title on a char boundary, and an
/// oversize icon dropped.
#[must_use]
pub fn fit_note(mut note: Note) -> Note {
    cut(&mut note.text, MAX_NOTE_TEXT);
    cut(&mut note.title, MAX_NOTE_TITLE);
    if note.icon.as_ref().is_some_and(|i| i.len() > MAX_ICON) {
        note.icon = None;
    }
    note
}

fn cut(s: &mut String, max: usize) {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
}

fn check(len: usize, max: usize) -> Result<(), LinkError> {
    if len > max {
        return Err(LinkError::TooBig { len, max });
    }
    Ok(())
}

fn check_note(n: &Note) -> Result<(), LinkError> {
    check(n.text.len(), MAX_NOTE_TEXT)?;
    check(n.title.len(), MAX_NOTE_TITLE)?;
    check(n.icon.as_ref().map_or(0, Vec::len), MAX_ICON)
}

fn check_frame(frame: &NotifyFrame) -> Result<(), LinkError> {
    match frame {
        NotifyFrame::Msg(NotifyMsg::Posted(n)) => check_note(n),
        NotifyFrame::Msg(NotifyMsg::Snapshot(notes)) => {
            check(notes.len(), MAX_SNAPSHOT)?;
            notes.iter().try_for_each(check_note)
        }
        NotifyFrame::Action(NotifyAction::Reply { text, .. }) => check(text.len(), MAX_REPLY),
        _ => Ok(()),
    }
}

/// Send `msg` on a new `Notify` lane; succeeds once the peer acknowledges it.
///
/// # Errors
///
/// [`LinkError`] if the lane is refused, the peer is gone or it refuses an
/// over-limit message (no acknowledgement arrives).
pub fn send_notify(conn: &Connection, msg: &NotifyMsg) -> Result<(), LinkError> {
    let frame = NotifyFrame::Msg(msg.clone());
    request::<_, ActionResult>(conn, LaneKind::Notify, &frame, NOTIFY_TIMEOUT)?;
    Ok(())
}

/// Send `action` on a new `Notify` lane and wait for the phone's verdict.
///
/// # Errors
///
/// [`LinkError`] if the lane is refused, the peer is gone or the answer is
/// late.
pub fn send_action(conn: &Connection, action: &NotifyAction) -> Result<ActionResult, LinkError> {
    let frame = NotifyFrame::Action(action.clone());
    request(conn, LaneKind::Notify, &frame, NOTIFY_TIMEOUT)
}

/// Read the one frame on an accepted `Notify` lane. A message is
/// acknowledged here; an action comes with the [`Responder`] to answer.
///
/// # Errors
///
/// [`LinkError::TooBig`] if the frame breaks a limit (the lane is dropped
/// unanswered); other [`LinkError`]s if no well-formed frame arrives in time.
pub fn recv_notify(stream: Stream) -> Result<Incoming, LinkError> {
    let (frame, responder) = serve::<NotifyFrame, ActionResult>(stream)?;
    check_frame(&frame)?;
    match frame {
        NotifyFrame::Msg(msg) => {
            responder.reply(&ActionResult::Done)?;
            Ok(Incoming::Msg(msg))
        }
        NotifyFrame::Action(action) => Ok(Incoming::Action(action, responder)),
    }
}
