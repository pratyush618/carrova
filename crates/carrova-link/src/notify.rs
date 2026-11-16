//! Phone notifications and the `Notify` lane.
//!
//! Wire (opener = sender), via [`request`]/[`serve`]: opener → `NotifyFrame`
//! (a message from the phone or an action from the laptop); the receiver
//! acknowledges a message with `()`, or answers an action with an
//! [`ActionResult`], and finishes. Over-limit frames are refused unanswered: text, title, reply and
//! snapshot length by the `MAX_*` constants, key, app id and app name at 256
//! bytes. An oversize icon is dropped, not refused.

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

/// Most icon bytes in one snapshot: 50 maximum icons would overrun the frame cap.
const SNAPSHOT_ICON_BUDGET: usize = 512 * 1024;
/// Longest note key, app id or app name, in bytes.
const MAX_ID: usize = 256;

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
    /// Done.
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

/// Fit a snapshot to the wire. The result is newest first and holds at most
/// [`MAX_SNAPSHOT`] notes, each through [`fit_note`]. Icons total at most
/// 512 KiB and are dropped from the oldest notes first, so a legal snapshot
/// always fits the transport's frame cap.
#[must_use]
pub fn fit_snapshot(mut notes: Vec<Note>) -> Vec<Note> {
    notes.sort_by_key(|n| std::cmp::Reverse(n.when_ms));
    notes.truncate(MAX_SNAPSHOT);
    let mut notes: Vec<Note> = notes.into_iter().map(fit_note).collect();
    let mut total: usize = notes
        .iter()
        .filter_map(|n| n.icon.as_ref())
        .map(Vec::len)
        .sum();
    for note in notes.iter_mut().rev() {
        if total <= SNAPSHOT_ICON_BUDGET {
            break;
        }
        if let Some(icon) = note.icon.take() {
            total -= icon.len();
        }
    }
    notes
}

/// Cut `note` to the wire limits: text and title on a char boundary, and an
/// oversize icon dropped.
#[must_use]
pub fn fit_note(mut note: Note) -> Note {
    cut(&mut note.text, MAX_NOTE_TEXT);
    cut(&mut note.title, MAX_NOTE_TITLE);
    drop_big_icon(&mut note);
    note
}

fn drop_big_icon(note: &mut Note) {
    if note.icon.as_ref().is_some_and(|i| i.len() > MAX_ICON) {
        note.icon = None;
    }
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
    check(n.key.len(), MAX_ID)?;
    check(n.app_id.len(), MAX_ID)?;
    check(n.app_name.len(), MAX_ID)
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
/// A snapshot is first fitted with [`fit_snapshot`].
///
/// # Errors
///
/// [`LinkError::TooBig`] (nothing sent) if a field breaks its limit; other
/// [`LinkError`]s if the lane is refused or the peer is gone.
pub fn send_notify(conn: &Connection, msg: &NotifyMsg) -> Result<(), LinkError> {
    let msg = match msg {
        NotifyMsg::Snapshot(notes) => NotifyMsg::Snapshot(fit_snapshot(notes.clone())),
        other => other.clone(),
    };
    let frame = NotifyFrame::Msg(msg);
    check_frame(&frame)?;
    request::<_, ()>(conn, LaneKind::Notify, &frame, NOTIFY_TIMEOUT)
}

/// Send `action` on a new `Notify` lane and wait for the phone's verdict.
///
/// # Errors
///
/// [`LinkError::TooBig`] (nothing sent) if a reply is over [`MAX_REPLY`];
/// other [`LinkError`]s if the lane is refused, the peer is gone or the
/// answer is late.
pub fn send_action(conn: &Connection, action: &NotifyAction) -> Result<ActionResult, LinkError> {
    let frame = NotifyFrame::Action(action.clone());
    check_frame(&frame)?;
    request(conn, LaneKind::Notify, &frame, NOTIFY_TIMEOUT)
}

/// Read the one frame on an accepted `Notify` lane. A message is
/// acknowledged here; an action comes with the [`Responder`] to answer.
/// An oversize icon is dropped and the note kept.
///
/// # Errors
///
/// [`LinkError::TooBig`] if the frame breaks a limit (the lane is dropped
/// unanswered); other [`LinkError`]s if no well-formed frame arrives in time.
pub fn recv_notify(stream: Stream) -> Result<Incoming, LinkError> {
    let (mut frame, pending) = serve::<NotifyFrame>(stream)?;
    check_frame(&frame)?;
    match &mut frame {
        NotifyFrame::Msg(NotifyMsg::Posted(n)) => drop_big_icon(n),
        NotifyFrame::Msg(NotifyMsg::Snapshot(notes)) => notes.iter_mut().for_each(drop_big_icon),
        _ => {}
    }
    match frame {
        NotifyFrame::Msg(msg) => {
            pending.ack()?;
            Ok(Incoming::Msg(msg))
        }
        NotifyFrame::Action(action) => Ok(Incoming::Action(action, pending.responder())),
    }
}
