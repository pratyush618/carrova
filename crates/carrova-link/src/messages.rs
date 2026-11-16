//! SMS over the `Messages` lane: the laptop pages threads and messages and
//! sends texts; the phone pushes incoming messages.
//!
//! Wire (opener = asker or pusher), frames are length + CBOR: opener →
//! `SmsFrame`; receiver → `Wire` and finishes. `Debug` on every type here hides
//! numbers, names and text.

use std::fmt;
use std::time::Duration;

use carrova_platform::phone::{SendOutcome, SmsMessage, SmsThread};
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
/// Longest destination number, in characters.
const MAX_NUMBER: usize = 32;
/// How long the laptop waits for the phone to answer (sending may take a while).
const REPLY_WAIT: Duration = Duration::from_secs(30);

/// What the laptop asks of the phone.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmsRequest {
    /// A page of threads, newest first, older than `before_ms`.
    Threads {
        /// Page cursor: only threads older than this.
        before_ms: Option<i64>,
    },
    /// A page of one thread's messages, older than `before_ms`.
    Thread {
        /// Thread id on the phone.
        id: i64,
        /// Page cursor: only messages older than this.
        before_ms: Option<i64>,
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

#[derive(Serialize, Deserialize)]
enum Wire {
    Reply(SmsReply),
    Ack,
}

hide!(SmsRequest, SmsReply, SmsPush, Wire);

/// What arrived on an accepted `Messages` lane.
pub enum SmsIncoming {
    /// A request to answer through the responder.
    Request(SmsRequest, SmsResponder),
    /// A push, already acknowledged.
    Push(SmsPush),
}

impl fmt::Debug for SmsIncoming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SmsIncoming(..)")
    }
}

/// The open lane a request came in on, waiting for its one reply.
#[derive(Debug)]
pub struct SmsResponder(Responder<Wire>);

impl SmsResponder {
    /// Send the reply and close the lane.
    ///
    /// # Errors
    ///
    /// [`LinkError::TooBig`] if the reply breaks a page or text limit;
    /// [`LinkError`] if the peer is gone.
    pub fn reply(self, reply: SmsReply) -> Result<(), LinkError> {
        check_reply(&reply)?;
        self.0.reply(&Wire::Reply(reply))
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

fn check_reply(reply: &SmsReply) -> Result<(), LinkError> {
    match reply {
        SmsReply::ThreadsPage(t) => cap(t.len(), THREAD_PAGE),
        SmsReply::ThreadPage(m) => {
            cap(m.len(), MESSAGE_PAGE)?;
            m.iter().try_for_each(|m| cap(m.text.len(), MAX_SMS_TEXT))
        }
        SmsReply::SendResult { .. } => Ok(()),
    }
}

fn check_push(SmsPush::Incoming(m): &SmsPush) -> Result<(), LinkError> {
    cap(m.text.len(), MAX_SMS_TEXT)
}

/// Ask the phone for threads or messages, or to send a text.
///
/// # Errors
///
/// [`LinkError::TooBig`] or [`LinkError::Protocol`] if the request or the
/// phone's reply breaks a limit; otherwise as [`reply::request`].
pub fn request_sms(conn: &Connection, req: &SmsRequest) -> Result<SmsReply, LinkError> {
    check_request(req)?;
    let frame = SmsFrame::Request(req.clone());
    match reply::request(conn, LaneKind::Messages, &frame, REPLY_WAIT)? {
        Wire::Reply(r) => check_reply(&r).map(|()| r),
        Wire::Ack => Err(LinkError::Protocol("reply")),
    }
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
    match reply::request(conn, LaneKind::Messages, &frame, REPLY_WAIT)? {
        Wire::Ack => Ok(()),
        Wire::Reply(_) => Err(LinkError::Protocol("reply")),
    }
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
    let (frame, responder) = reply::serve::<SmsFrame, Wire>(stream)?;
    match frame {
        SmsFrame::Request(req) => {
            check_request(&req)?;
            Ok(SmsIncoming::Request(req, SmsResponder(responder)))
        }
        SmsFrame::Push(push) => {
            check_push(&push)?;
            responder.reply(&Wire::Ack)?;
            Ok(SmsIncoming::Push(push))
        }
    }
}
