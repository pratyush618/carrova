//! Parsing the JSON the Kotlin glue hands over: phone events pulled from
//! `Events.take()` and the lists the glue returns. Pure, so it is tested
//! on any host.

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::PlatformError;
use crate::phone::{CallState, Note, PhoneEvent, SmsMessage};

/// An event as `Events.kt` writes it: `{"type": "...", fields...}`.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Wire {
    NotePosted(Note),
    NoteRemoved {
        key: String,
    },
    SmsReceived(SmsMessage),
    CallIdle,
    CallRinging {
        number: String,
        name: Option<String>,
    },
    CallActive {
        since_ms: i64,
        muted: bool,
    },
    CallEnded {
        missed: bool,
    },
    Dropped {
        count: u32,
    },
    #[serde(other)]
    Unknown,
}

/// One event from `Events.take()`; `None` for a `type` this build does not
/// know (skipped, not an error, so a newer glue never ends the watch).
///
/// # Errors
///
/// [`PlatformError::Android`] if `json` is not a well-formed event.
pub(super) fn parse(json: &str) -> Result<Option<PhoneEvent>, PlatformError> {
    let event = match from_json("Events.take", json)? {
        Wire::NotePosted(note) => PhoneEvent::NotePosted(note),
        Wire::NoteRemoved { key } => PhoneEvent::NoteRemoved { key },
        Wire::SmsReceived(sms) => PhoneEvent::SmsReceived(sms),
        Wire::CallIdle => PhoneEvent::Call(CallState::Idle),
        Wire::CallRinging { number, name } => PhoneEvent::Call(CallState::Ringing { number, name }),
        // Whether audio runs through a laptop is the link's to say, not the phone's.
        Wire::CallActive { since_ms, muted } => PhoneEvent::Call(CallState::Active {
            since_ms,
            muted,
            on_laptop_audio: false,
        }),
        Wire::CallEnded { missed } => PhoneEvent::Call(CallState::Ended { missed }),
        Wire::Dropped { count } => PhoneEvent::Dropped { count },
        Wire::Unknown => return Ok(None),
    };
    Ok(Some(event))
}

/// The JSON array the glue's `call` returned.
///
/// # Errors
///
/// [`PlatformError::Android`] if `json` is not an array of `T`.
pub(super) fn list<T: DeserializeOwned>(
    call: &'static str,
    json: &str,
) -> Result<Vec<T>, PlatformError> {
    from_json(call, json)
}

/// `json` as `T`; the error names the position only, never the content
/// (serde's messages quote the offending value).
fn from_json<T: DeserializeOwned>(call: &'static str, json: &str) -> Result<T, PlatformError> {
    serde_json::from_str(json).map_err(|e| PlatformError::Android {
        call,
        cause: format!("bad JSON ({:?}) at column {}", e.classify(), e.column()),
    })
}

#[cfg(test)]
#[path = "../../tests/android/events/unit.rs"]
mod tests;
