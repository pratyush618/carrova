//! Parsing the JSON the Kotlin glue hands over: phone events pulled from
//! `Events.take()` and the lists the glue returns. Pure, so it is tested
//! on any host.

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::PlatformError;
use crate::phone::{CallState, Note, PhoneEvent, PhonePerm, SmsMessage};

/// An event as `Events.kt` writes it: `{"type": "...", fields...}`.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Wire {
    NotePosted(Note),
    NoteRemoved {
        key: String,
    },
    SmsReceived(SmsMessage),
    SmsStored {
        thread_id: i64,
    },
    CallIdle,
    CallRinging {
        number: String,
        name: Option<String>,
    },
    CallActive {
        since_ms: i64,
        muted: bool,
        #[serde(default)]
        bt: bool,
        #[serde(default)]
        outgoing: bool,
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
        Wire::SmsStored { thread_id } => PhoneEvent::SmsStored { thread_id },
        Wire::CallIdle => PhoneEvent::Call(CallState::Idle),
        Wire::CallRinging { number, name } => PhoneEvent::Call(CallState::Ringing { number, name }),
        // A Bluetooth SCO device can't be told apart from a headset; either way the speaker stays off.
        Wire::CallActive {
            since_ms,
            muted,
            bt,
            outgoing,
        } => PhoneEvent::Call(CallState::Active {
            since_ms,
            muted,
            on_laptop_audio: bt,
            outgoing,
        }),
        Wire::CallEnded { missed } => PhoneEvent::Call(CallState::Ended { missed }),
        Wire::Dropped { count } => PhoneEvent::Dropped { count },
        Wire::Unknown => return Ok(None),
    };
    Ok(Some(event))
}

/// The call state `Calls.state()` returned (in the event format).
///
/// # Errors
///
/// [`PlatformError::Android`] if `json` is not a call event.
pub(super) fn call_state(json: &str) -> Result<CallState, PlatformError> {
    match parse(json)? {
        // `Calls.state` reports what is, never a call that ended.
        Some(PhoneEvent::Call(state)) if !matches!(state, CallState::Ended { .. }) => Ok(state),
        _ => Err(PlatformError::Android {
            call: "Calls.state",
            cause: "not a call state".into(),
        }),
    }
}

/// `err` as [`PlatformError::PermissionDenied`] for `perm` when Java threw a
/// `SecurityException` (a permission revoked after the check); else as is.
pub(super) fn denied_as(perm: PhonePerm, err: PlatformError) -> PlatformError {
    match err {
        PlatformError::Android { ref cause, .. } if cause.ends_with("SecurityException") => {
            PlatformError::PermissionDenied(perm)
        }
        err => err,
    }
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
pub(super) fn from_json<T: DeserializeOwned>(
    call: &'static str,
    json: &str,
) -> Result<T, PlatformError> {
    serde_json::from_str(json).map_err(|e| PlatformError::Android {
        call,
        cause: format!("bad JSON ({:?}) at column {}", e.classify(), e.column()),
    })
}

#[cfg(test)]
#[path = "../../tests/android/events/unit.rs"]
mod tests;
