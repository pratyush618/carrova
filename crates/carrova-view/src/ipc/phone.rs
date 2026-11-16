//! What the front-end asks of a linked phone and how it went: the
//! commands' own types, mapped one to one onto the link's. The mapping
//! is exhaustive one way only: a new `NotifyAction` or `CallCommand`
//! variant is not exposed to the UI until it is added here.

use carrova_link::{ActionResult, CallCommand, CallReply, NotifyAction};
use carrova_platform::phone::AudioSide;
use serde::{Deserialize, Serialize};

/// What to do with one of the phone's notifications. `Debug` prints no
/// content.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoteAction {
    /// Send an inline reply.
    Reply {
        /// Notification key.
        key: String,
        /// Reply text.
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

private_debug!(NoteAction);

impl From<NoteAction> for NotifyAction {
    fn from(action: NoteAction) -> Self {
        match action {
            NoteAction::Reply { key, text } => Self::Reply { key, text },
            NoteAction::Dismiss { key } => Self::Dismiss { key },
            NoteAction::Open { key } => Self::Open { key },
        }
    }
}

/// How a [`NoteAction`] went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum NoteOutcome {
    /// Done.
    Done,
    /// The notification is gone from the phone.
    Gone,
    /// The phone could not do it.
    Failed,
}

impl From<ActionResult> for NoteOutcome {
    fn from(result: ActionResult) -> Self {
        match result {
            ActionResult::Done => Self::Done,
            ActionResult::Gone => Self::Gone,
            ActionResult::Failed => Self::Failed,
        }
    }
}

/// Something to do with the phone's call. `Debug` prints no number.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CallAction {
    /// Answer the ringing call.
    Answer,
    /// Decline the ringing call.
    Decline,
    /// Hang up.
    End,
    /// Mute or unmute the microphone.
    Mute {
        /// Muted.
        on: bool,
    },
    /// Call `number` (`[0-9+*#]`, 1 to 32 characters) from the phone.
    Dial {
        /// Number to call.
        number: String,
    },
    /// Send keypad tones (as `Dial`), through this computer's call audio.
    Tones {
        /// Keys pressed.
        digits: String,
    },
    /// Move the call's audio.
    AudioTo {
        /// Where to.
        side: AudioSide,
    },
}

private_debug!(CallAction);

impl From<CallAction> for CallCommand {
    fn from(action: CallAction) -> Self {
        match action {
            CallAction::Answer => Self::Answer,
            CallAction::Decline => Self::Decline,
            CallAction::End => Self::End,
            CallAction::Mute { on } => Self::Mute(on),
            CallAction::Dial { number } => Self::Dial { number },
            CallAction::Tones { digits } => Self::Tones(digits),
            CallAction::AudioTo { side } => Self::AudioTo(side),
        }
    }
}

/// How a [`CallAction`] went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CallOutcome {
    /// Done.
    Done,
    /// Answer or decline with nothing ringing.
    NotRinging,
    /// It needs a call and there is none.
    NoCall,
    /// It could not be done.
    Failed,
}

impl From<CallReply> for CallOutcome {
    /// The outcome of an action's `reply`; a history page answers no
    /// action, so it reads as failed.
    fn from(reply: CallReply) -> Self {
        match reply {
            CallReply::Done => Self::Done,
            CallReply::NotRinging => Self::NotRinging,
            CallReply::NoCall => Self::NoCall,
            CallReply::Failed | CallReply::HistoryPage(_) => Self::Failed,
        }
    }
}
