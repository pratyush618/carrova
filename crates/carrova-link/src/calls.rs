//! Phone call state and call commands on the `Calls` lane.
//!
//! Wire (opener = asker), frames are length + CBOR, first frame a [`CallFrame`]:
//! `State` (phone → laptop) is acknowledged with `()`; `Command` (laptop →
//! phone) is answered with one [`CallReply`]. Whether a command is allowed in
//! the current call state is the controller's call, not this lane's.
//!
//! Older peers: a `Dial` without `sim` uses the phone's default SIM, and
//! an older phone ignores `sim`; it can't answer [`CallCommand::Sims`]
//! (the lane fails), so the laptop then offers no SIM choice. Nor can it
//! answer [`CallCommand::Contacts`]: the laptop then suggests no contacts.

use std::fmt;
use std::time::Duration;

use carrova_platform::phone::{AudioSide, CallRecord, CallState, Contact, Cursor, Sim};
use carrova_transport::Stream;
use serde::{Deserialize, Serialize};

use crate::LinkError;
use crate::lane::{FRAME_TIMEOUT, LaneKind};
use crate::messages::{check_contacts, check_sims};
use crate::reply::{Pending, Responder, request, serve};

/// Most records in one [`CallReply::HistoryPage`].
pub const HISTORY_PAGE: usize = 50;

/// How long the laptop waits for the phone to carry out a command.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Longest dialled number or tone string.
const MAX_NUMBER: usize = 32;

/// Most contacts in one [`CallReply::Contacts`].
pub const MAX_CONTACTS: usize = 10;

/// Longest contacts search, in bytes.
const MAX_QUERY: usize = 64;

/// Something the laptop asks of the call. `Debug` hides numbers.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallCommand {
    /// Answer the ringing call.
    Answer,
    /// Decline the ringing call.
    Decline,
    /// Hang up.
    End,
    /// Mute or unmute the microphone.
    Mute(bool),
    /// Place a call; `number` is `[0-9+*#]`, 1 to 32 characters.
    Dial {
        /// Number to call.
        number: String,
        /// The SIM to call from ([`Sim::id`]); `None`: the phone's default.
        #[serde(default)]
        sim: Option<i32>,
    },
    /// The phone's active SIMs, to choose one to call from.
    Sims,
    /// Contact numbers whose name or number matches `query` (1 to 64
    /// bytes, no control characters).
    Contacts {
        /// What was typed.
        query: String,
    },
    /// One page of the call log, newest first by `(when_ms, id)`, after `before`.
    History {
        /// Page cursor: only entries ordered strictly before this one.
        before: Option<Cursor>,
    },
    /// Send keypad tones (same alphabet as `Dial`). Answered by the laptop's
    /// own platform, never sent to the phone.
    Tones(String),
    /// Move call audio. Answered by the laptop's own platform, never sent to
    /// the phone.
    AudioTo(AudioSide),
}

impl CallCommand {
    /// Reject a laptop-local command: it never crosses the wire.
    fn check_remote(&self) -> Result<(), LinkError> {
        match self {
            Self::Tones(_) | Self::AudioTo(_) => Err(LinkError::Protocol("local")),
            _ => Ok(()),
        }
    }

    /// Reject a malformed number before it goes anywhere.
    fn check(&self) -> Result<(), LinkError> {
        match self {
            Self::Dial { number, .. } | Self::Tones(number) => check_number(number),
            Self::Contacts { query } => check_query(query),
            _ => Ok(()),
        }
    }
}

impl fmt::Debug for CallCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Answer => "CallCommand::Answer",
            Self::Decline => "CallCommand::Decline",
            Self::End => "CallCommand::End",
            Self::Mute(_) => "CallCommand::Mute(..)",
            Self::Dial { .. } => "CallCommand::Dial(..)",
            Self::History { .. } => "CallCommand::History(..)",
            Self::Sims => "CallCommand::Sims",
            Self::Contacts { .. } => "CallCommand::Contacts(..)",
            Self::Tones(_) => "CallCommand::Tones(..)",
            Self::AudioTo(_) => "CallCommand::AudioTo(..)",
        })
    }
}

/// The phone's answer to a [`CallCommand`]. `Debug` hides history entries.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallReply {
    /// The command was carried out.
    Done,
    /// `Answer` or `Decline` with nothing ringing.
    NotRinging,
    /// The command needs a call and there is none.
    NoCall,
    /// The phone could not do it.
    Failed,
    /// The requested page of the call log (at most [`HISTORY_PAGE`]).
    HistoryPage(Vec<CallRecord>),
    /// The phone's active SIMs, by slot (at most
    /// [`MAX_SIMS`](crate::MAX_SIMS)); empty when it may not read them.
    Sims(Vec<Sim>),
    /// Contacts matching a search (at most [`MAX_CONTACTS`]); empty when
    /// it may not read them.
    Contacts(Vec<Contact>),
}

impl fmt::Debug for CallReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Done => "CallReply::Done",
            Self::NotRinging => "CallReply::NotRinging",
            Self::NoCall => "CallReply::NoCall",
            Self::Failed => "CallReply::Failed",
            Self::HistoryPage(_) => "CallReply::HistoryPage(..)",
            Self::Sims(_) => "CallReply::Sims(..)",
            Self::Contacts(_) => "CallReply::Contacts(..)",
        })
    }
}

/// First frame of a `Calls` lane.
#[derive(Serialize, Deserialize)]
enum CallFrame {
    State(CallState),
    Command(CallCommand),
}

/// What arrived on an accepted `Calls` lane. `Debug` hides numbers and names.
pub enum CallIncoming {
    /// The phone's call state changed; acknowledge it through the
    /// [`Pending`] once applied, so the next state waits for this one.
    State(CallState, Pending),
    /// The laptop asks for something; answer through the [`Responder`].
    Command(CallCommand, Responder<CallReply>),
}

impl fmt::Debug for CallIncoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::State(..) => "CallIncoming::State(..)",
            Self::Command(..) => "CallIncoming::Command(..)",
        })
    }
}

/// Only `[0-9+*#]`, 1 to 32 characters.
fn check_number(number: &str) -> Result<(), LinkError> {
    let ok = (1..=MAX_NUMBER).contains(&number.len())
        && number
            .bytes()
            .all(|b| b.is_ascii_digit() || b"+*#".contains(&b));
    ok.then_some(()).ok_or(LinkError::Protocol("number"))
}

/// 1 to [`MAX_QUERY`] bytes, no control characters.
fn check_query(query: &str) -> Result<(), LinkError> {
    let ok = (1..=MAX_QUERY).contains(&query.len()) && !query.chars().any(char::is_control);
    ok.then_some(()).ok_or(LinkError::Protocol("query"))
}

/// Report the phone's call state on a new `Calls` lane; succeeds once the
/// peer acknowledges it.
///
/// # Errors
///
/// [`LinkError`] if the lane is refused or the peer is gone or silent.
pub fn send_call_state(
    conn: &carrova_transport::Connection,
    state: &CallState,
) -> Result<(), LinkError> {
    request(
        conn,
        LaneKind::Calls,
        &CallFrame::State(state.clone()),
        FRAME_TIMEOUT,
    )
}

/// Ask the phone to carry out `command` and wait for its reply.
///
/// # Errors
///
/// [`LinkError::Protocol`] (`"local"`) for `Tones` and `AudioTo`, which the
/// laptop answers itself; (`"number"`) for a malformed `Dial` number;
/// (`"query"`) for a malformed `Contacts` search; (`"history"`) for an oversized history page;
/// [`LinkError::TooBig`] for an oversized SIM or contacts list; otherwise
/// [`LinkError`] if the lane is refused or the peer is gone or silent.
pub fn call_command(
    conn: &carrova_transport::Connection,
    command: &CallCommand,
) -> Result<CallReply, LinkError> {
    command.check_remote()?;
    command.check()?;
    let reply: CallReply = request(
        conn,
        LaneKind::Calls,
        &CallFrame::Command(command.clone()),
        COMMAND_TIMEOUT,
    )?;
    match &reply {
        CallReply::HistoryPage(page) if page.len() > HISTORY_PAGE => {
            Err(LinkError::Protocol("history"))
        }
        CallReply::Sims(sims) => check_sims(sims).map(|()| reply),
        CallReply::Contacts(contacts) => check_contacts(contacts, MAX_CONTACTS).map(|()| reply),
        _ => Ok(reply),
    }
}

/// Read the first frame of an accepted `Calls` lane. A state comes with its
/// [`Pending`] acknowledgement; a command with the [`Responder`] to answer
/// it.
///
/// # Errors
///
/// [`LinkError::Protocol`] (`"local"`) for `Tones` and `AudioTo`, which never
/// cross the wire; (`"number"`) for a malformed `Dial` number; (`"query"`)
/// for a malformed `Contacts` search; otherwise [`LinkError`] if no well-formed frame arrives in time.
pub fn recv_calls(stream: Stream) -> Result<CallIncoming, LinkError> {
    let (frame, pending) = serve(stream)?;
    match frame {
        CallFrame::Command(command) => {
            command.check_remote()?;
            command.check()?;
            Ok(CallIncoming::Command(command, pending.responder()))
        }
        CallFrame::State(state) => Ok(CallIncoming::State(state, pending)),
    }
}
