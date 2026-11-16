//! SMS over the `Messages` lane: the laptop pages threads and messages and
//! sends texts; the phone pushes incoming messages.
//!
//! Wire (opener = asker or pusher), frames are length + CBOR: opener →
//! `SmsFrame`; receiver → `SmsReply` (request) or `()` (push) and finishes.
//! `Debug` on every type here hides numbers, names and text. Receivers cap
//! pages at [`THREAD_PAGE`]/[`MESSAGE_PAGE`], texts and snippets at
//! [`MAX_SMS_TEXT`], names and addresses at 256 bytes, and a destination
//! number at 32 characters of `[0-9+*#]`.

use std::fmt;
use std::time::Duration;

use carrova_platform::phone::{Cursor, SendOutcome, SmsMessage, SmsThread};
use carrova_transport::Connection;
use serde::{Deserialize, Serialize};

use crate::LinkError;
use crate::lane::LaneKind;
use crate::reply::{self, Responder};
use carrova_transport::Stream;

/// Threads per page.
pub const THREAD_PAGE: usize = 30;
/// Messages per page.
pub const MESSAGE_PAGE: usize = 50;
/// Longest SMS text (bytes) either side accepts.
pub const MAX_SMS_TEXT: usize = 4096;
/// Longest contact name or number on a thread, in bytes.
const MAX_NAME: usize = 256;
/// Longest destination number, in characters.
const MAX_NUMBER: usize = 32;
/// How long the laptop waits for the phone to answer (sending may take a while).
const REPLY_WAIT: Duration = Duration::from_secs(30);

/// What the laptop asks of the phone.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmsRequest {
    /// A page of threads, newest first by `(when_ms, id)`, after `before`.
    Threads {
        /// Page cursor: only threads ordered strictly before this one.
        before: Option<Cursor>,
    },
    /// A page of one thread's messages, newest first by `(when_ms, id)`,
    /// after `before`.
    Thread {
        /// Thread id on the phone.
        id: i64,
        /// Page cursor: only messages ordered strictly before this one.
        before: Option<Cursor>,
    },
    /// Send a text.
    Send {
        /// Destination number.
        to: String,
        /// Message text.
        text: String,
        /// Caller's id, echoed in the result.
        client_id: u64,
    },
}

/// The phone's answer to an [`SmsRequest`].
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmsReply {
    /// At most [`THREAD_PAGE`] threads.
    ThreadsPage(Vec<SmsThread>),
    /// At most [`MESSAGE_PAGE`] messages.
    ThreadPage(Vec<SmsMessage>),
    /// How a send went.
    SendResult {
        /// The request's `client_id`.
        client_id: u64,
        /// What happened.
        outcome: SendOutcome,
    },
}

/// Unsolicited phone → laptop news.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmsPush {
    /// A message arrived.
    Incoming(SmsMessage),
    /// The phone stored an incoming message: refetch that thread.
    Stored {
        /// The thread that got it.
        thread_id: i64,
    },
    /// SMS changed in ways the laptop may have missed (the phone's event
    /// queue overflowed): refetch the threads and any open thread.
    Resync,
}

macro_rules! hide {
    ($($t:ty),*) => {$(
        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($t), "(..)"))
            }
        }
    )*};
}

#[derive(Serialize, Deserialize)]
enum SmsFrame {
    Request(SmsRequest),
    Push(SmsPush),
}

hide!(SmsRequest, SmsReply, SmsPush);

/// What arrived on an accepted `Messages` lane.
pub enum SmsIncoming {
    /// A request to answer through the responder.
    Request(SmsRequest, Responder<SmsReply>),
    /// A push, already acknowledged.
    Push(SmsPush),
}

impl fmt::Debug for SmsIncoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SmsIncoming(..)")
    }
}

fn cap(len: usize, max: usize) -> Result<(), LinkError> {
    if len > max {
        return Err(LinkError::TooBig { len, max });
    }
    Ok(())
}

fn check_request(req: &SmsRequest) -> Result<(), LinkError> {
    if let SmsRequest::Send { to, text, .. } = req {
        cap(text.len(), MAX_SMS_TEXT)?;
        let plausible = !to.is_empty()
            && to.len() <= MAX_NUMBER
            && to
                .bytes()
                .all(|b| b.is_ascii_digit() || b"+*#".contains(&b));
        if !plausible {
            return Err(LinkError::Protocol("number"));
        }
    }
    Ok(())
}

/// Check a reply against the page and text limits; the phone calls this
/// before [`Responder::reply`], and [`request_sms`] on what it receives.
///
/// # Errors
///
/// [`LinkError::TooBig`] if a page, text, snippet, name or address is over its cap.
pub fn check_sms_reply(reply: &SmsReply) -> Result<(), LinkError> {
    match reply {
        SmsReply::ThreadsPage(t) => {
            cap(t.len(), THREAD_PAGE)?;
            t.iter().try_for_each(|t| {
                cap(t.name.as_deref().map_or(0, str::len), MAX_NAME)?;
                cap(t.address.len(), MAX_NAME)?;
                cap(t.snippet.len(), MAX_SMS_TEXT)
            })
        }
        SmsReply::ThreadPage(m) => {
            cap(m.len(), MESSAGE_PAGE)?;
            m.iter().try_for_each(|m| cap(m.text.len(), MAX_SMS_TEXT))
        }
        SmsReply::SendResult { .. } => Ok(()),
    }
}

/// A reply must be the kind its request asks for.
fn matches(req: &SmsRequest, reply: &SmsReply) -> bool {
    match (req, reply) {
        (SmsRequest::Threads { .. }, SmsReply::ThreadsPage(_))
        | (SmsRequest::Thread { .. }, SmsReply::ThreadPage(_)) => true,
        (SmsRequest::Send { client_id, .. }, SmsReply::SendResult { client_id: got, .. }) => {
            client_id == got
        }
        _ => false,
    }
}

fn check_push(push: &SmsPush) -> Result<(), LinkError> {
    match push {
        SmsPush::Incoming(m) => cap(m.text.len(), MAX_SMS_TEXT),
        SmsPush::Stored { .. } | SmsPush::Resync => Ok(()),
    }
}

/// Ask the phone for threads or messages, or to send a text.
///
/// # Errors
///
/// [`LinkError::TooBig`] or [`LinkError::Protocol`] if the request or the
/// phone's reply breaks a limit or is not the kind asked for; otherwise as [`reply::request`].
pub fn request_sms(conn: &Connection, req: &SmsRequest) -> Result<SmsReply, LinkError> {
    check_request(req)?;
    let frame = SmsFrame::Request(req.clone());
    let reply: SmsReply = reply::request(conn, LaneKind::Messages, &frame, REPLY_WAIT)?;
    check_sms_reply(&reply)?;
    if !matches(req, &reply) {
        return Err(LinkError::Protocol("reply"));
    }
    Ok(reply)
}

/// Push `push` to the laptop; succeeds once it acknowledges.
///
/// # Errors
///
/// [`LinkError::TooBig`] if the text is over [`MAX_SMS_TEXT`]; otherwise as
/// [`reply::request`].
pub fn push_sms(conn: &Connection, push: &SmsPush) -> Result<(), LinkError> {
    check_push(push)?;
    let frame = SmsFrame::Push(push.clone());
    reply::request::<_, ()>(conn, LaneKind::Messages, &frame, REPLY_WAIT)
}

/// Read what the peer opened on an accepted `Messages` lane. A push is
/// acknowledged here; a request is answered later through its responder.
///
/// # Errors
///
/// [`LinkError::TooBig`] for text over [`MAX_SMS_TEXT`];
/// [`LinkError::Protocol`] for an implausible destination number; otherwise
/// as [`reply::serve`].
pub fn recv_sms(stream: Stream) -> Result<SmsIncoming, LinkError> {
    let (frame, pending) = reply::serve::<SmsFrame>(stream)?;
    match frame {
        SmsFrame::Request(req) => {
            check_request(&req)?;
            Ok(SmsIncoming::Request(req, pending.responder()))
        }
        SmsFrame::Push(push) => {
            check_push(&push)?;
            pending.ack()?;
            Ok(SmsIncoming::Push(push))
        }
    }
}
