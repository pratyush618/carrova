//! The phone's notifications as this computer shows them.

use carrova_platform::phone::{Hidden, Note};
use data_encoding::BASE64;
use serde::Serialize;

/// One notification. `Debug` prints no content.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NoteView {
    /// Android notification key: what actions name.
    pub key: String,
    /// Package id of the posting app.
    pub app_id: String,
    /// The app's display name.
    pub app_name: String,
    /// Title (empty when withheld).
    pub title: String,
    /// Text (empty when withheld).
    pub text: String,
    /// When it was posted (ms since the Unix epoch).
    pub when_ms: i64,
    /// Whether it offers an inline reply.
    pub can_reply: bool,
    /// The app icon as a `data:image/png;base64,…` URL, ready for an
    /// `<img src>`; `None` when the phone sent none.
    pub icon: Option<String>,
    /// Why some content was withheld; the UI picks the copy.
    pub hidden: Option<Hidden>,
}

private_debug!(NoteView);

/// The notifications list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NotesView {
    /// This computer is locked: only how many there are ("<Phone>: N new
    /// notifications").
    Locked {
        /// Notifications showing on the phone.
        count: usize,
    },
    /// Every notification, newest first.
    List {
        /// Newest first.
        notes: Vec<NoteView>,
    },
}

/// The view of `notes` (newest first); `locked`: this computer is locked
/// and their content withheld ([`withheld_while_locked`]).
#[must_use]
pub fn notes_view(notes: &[Note], locked: bool) -> NotesView {
    if locked {
        return NotesView::Locked { count: notes.len() };
    }
    NotesView::List {
        notes: notes.iter().map(note_view).collect(),
    }
}

/// Whether the engine withheld `notes`' content because this computer is
/// (or may be) locked: the `locked` input of [`notes_view`].
#[must_use]
pub fn withheld_while_locked(notes: &[Note]) -> bool {
    notes.iter().any(|n| n.hidden == Some(Hidden::Locked))
}

fn note_view(note: &Note) -> NoteView {
    NoteView {
        key: note.key.clone(),
        app_id: note.app_id.clone(),
        app_name: note.app_name.clone(),
        title: note.title.clone(),
        text: note.text.clone(),
        when_ms: note.when_ms,
        can_reply: note.can_reply,
        icon: note
            .icon
            .as_deref()
            .map(|png| format!("data:image/png;base64,{}", BASE64.encode(png))),
        hidden: note.hidden,
    }
}
