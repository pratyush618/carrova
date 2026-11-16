//! Phone notifications and the `Notify` lane.
//!
//! Wire (opener = sender), via [`request`]/[`serve`]: opener → `NotifyFrame`
//! (a message from the phone, or an action or app-list request from the
//! laptop); the receiver acknowledges a message with `()`, answers an action
//! with an [`ActionResult`] or a list request with its [`AppEntry`] list,
//! and finishes. Over-limit frames are refused unanswered: text, title, reply,
//! snapshot and app-list length by the `MAX_*` constants, key, app id and app
//! name at 256 bytes. An oversize icon is dropped, not refused.

use std::fmt;
use std::time::Duration;

use carrova_platform::phone::Note;
use carrova_transport::{Connection, Stream};
use serde::{Deserialize, Serialize};

use crate::LinkError;
use crate::lane::LaneKind;
use crate::reply::{Pending, Responder, request, serve};

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
/// Most apps in the phone's per-app list.
pub const MAX_APPS: usize = 500;
/// Longest app name in that list, in bytes.
pub const MAX_APP_NAME: usize = MAX_ID;

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
    /// Whether the phone has notification access now: without it nothing
    /// is mirrored. Sent when it changes, and to a device that starts
    /// taking notifications while it is missing.
    Access {
        /// Access held.
        granted: bool,
    },
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
    /// Remind the user on the phone to grant notification access (a local
    /// notification that opens the settings page).
    Remind,
}

/// One app in the phone's per-app notification list. `Debug` prints no
/// names or ids.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEntry {
    /// Package id, at most 256 bytes.
    pub app_id: String,
    /// Display name, at most [`MAX_APP_NAME`] bytes.
    pub app_name: String,
    /// Whether its notifications reach linked devices.
    pub on: bool,
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
    ListApps,
}

/// What arrived on an accepted `Notify` lane.
#[derive(Debug)]
pub enum Incoming {
    /// A message; acknowledge it through the [`Pending`] once applied, so
    /// the sender's next message waits for this one.
    Msg(NotifyMsg, Pending),
    /// An action to carry out, then answer through the [`Responder`].
    Action(NotifyAction, Responder<ActionResult>),
    /// A request for the per-app list: answer with [`fit_apps`]'s result.
    ListApps(Responder<Vec<AppEntry>>),
}

// Debug never prints titles, text or reply bodies.
impl fmt::Debug for NotifyMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Posted(_) => "NotifyMsg::Posted(..)",
            Self::Removed { .. } => "NotifyMsg::Removed(..)",
            Self::Snapshot(_) => "NotifyMsg::Snapshot(..)",
            Self::Access { .. } => "NotifyMsg::Access(..)",
        })
    }
}

impl fmt::Debug for NotifyAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Reply { .. } => "NotifyAction::Reply(..)",
            Self::Dismiss { .. } => "NotifyAction::Dismiss(..)",
            Self::Open { .. } => "NotifyAction::Open(..)",
            Self::Remind => "NotifyAction::Remind",
        })
    }
}

impl fmt::Debug for AppEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AppEntry(..)")
    }
}

/// Fit an app list to the wire: entries whose id is over 256 bytes are
/// dropped, names cut on a char boundary, and at most [`MAX_APPS`] kept.
#[must_use]
pub fn fit_apps(apps: Vec<AppEntry>) -> Vec<AppEntry> {
    apps.into_iter()
        .filter(|a| a.app_id.len() <= MAX_ID)
        .take(MAX_APPS)
        .map(|mut a| {
            cut(&mut a.app_name, MAX_APP_NAME);
            a
        })
        .collect()
}

fn check_apps(apps: &[AppEntry]) -> Result<(), LinkError> {
    check(apps.len(), MAX_APPS)?;
    apps.iter().try_for_each(|a| {
        check(a.app_id.len(), MAX_ID)?;
        check(a.app_name.len(), MAX_APP_NAME)
    })
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
        NotifyFrame::Msg(NotifyMsg::Removed { key })
        | NotifyFrame::Action(NotifyAction::Dismiss { key } | NotifyAction::Open { key }) => {
            check(key.len(), MAX_ID)
        }
        NotifyFrame::Action(NotifyAction::Reply { key, text }) => {
            check(key.len(), MAX_ID)?;
            check(text.len(), MAX_REPLY)
        }
        NotifyFrame::Msg(NotifyMsg::Access { .. })
        | NotifyFrame::Action(NotifyAction::Remind)
        | NotifyFrame::ListApps => Ok(()),
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

/// Ask the phone for its per-app notification list.
///
/// # Errors
///
/// [`LinkError::TooBig`] if the list breaks a limit (see [`fit_apps`]);
/// other [`LinkError`]s if the lane is refused, the peer is gone or the
/// answer is late.
pub fn list_apps(conn: &Connection) -> Result<Vec<AppEntry>, LinkError> {
    let apps: Vec<AppEntry> = request(
        conn,
        LaneKind::Notify,
        &NotifyFrame::ListApps,
        NOTIFY_TIMEOUT,
    )?;
    check_apps(&apps)?;
    Ok(apps)
}

/// Read the one frame on an accepted `Notify` lane. A message comes with
/// its [`Pending`] acknowledgement; an action with the [`Responder`] to
/// answer.
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
        NotifyFrame::Msg(msg) => Ok(Incoming::Msg(msg, pending)),
        NotifyFrame::Action(action) => Ok(Incoming::Action(action, pending.responder())),
        NotifyFrame::ListApps => Ok(Incoming::ListApps(pending.responder())),
    }
}
