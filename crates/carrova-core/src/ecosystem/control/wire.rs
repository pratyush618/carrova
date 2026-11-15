//! The control socket's messages and framing: a big-endian `u32` length,
//! then that many bytes of CBOR.

use std::io::{self, Read, Write};
use std::path::PathBuf;

use carrova_crypto::identity::DeviceId;
use carrova_link::{ClipItem, Feature, FeatureState};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::ecosystem::{Answer, AskId, HandoffItem, LinkedPeer};
use crate::error::Result;

/// Largest message the supervisor reads (a request), in bytes.
pub const MAX_CONTROL_MESSAGE: u32 = 64 * 1024;
/// Largest message a client reads (an answer or an event), in bytes: the
/// clip history holds up to 20 clips of up to 10 MiB.
pub(super) const MAX_CONTROL_REPLY: u32 = 256 << 20;

/// What a client asks the supervisor (its [`super::Supervisor`] call of
/// the same name). Its `Debug` leaves out paths.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlRequest {
    /// [`crate::ecosystem::Supervisor::status`].
    Status,
    /// [`crate::ecosystem::Supervisor::ping`].
    Ping(DeviceId),
    /// [`crate::ecosystem::Supervisor::send_clipboard`].
    SendClipboard(DeviceId),
    /// [`crate::ecosystem::Supervisor::send_file`]; the path must be absolute and name
    /// a regular file (not a symlink).
    SendFile(DeviceId, PathBuf),
    /// [`crate::ecosystem::Supervisor::handoff`]; a file as for [`Self::SendFile`].
    Handoff(DeviceId, HandoffItem),
    /// [`crate::ecosystem::Supervisor::ring`].
    Ring(DeviceId),
    /// [`crate::ecosystem::Supervisor::stop_ring`].
    StopRing(DeviceId),
    /// [`crate::ecosystem::Supervisor::answer`].
    Answer(AskId, Answer),
    /// [`crate::ecosystem::Supervisor::set_feature`].
    SetFeature(DeviceId, Feature, FeatureState),
    /// [`crate::ecosystem::Supervisor::clip_history`].
    ClipHistory,
    /// [`crate::ecosystem::Supervisor::clear_clip_history`].
    ClearClipHistory,
    /// Stream the supervisor's events: answered [`ControlResponse::Done`],
    /// then the connection carries only [`crate::Event`] messages until
    /// either side closes it.
    Subscribe,
}

impl std::fmt::Debug for ControlRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status => f.write_str("Status"),
            Self::ClipHistory => f.write_str("ClipHistory"),
            Self::ClearClipHistory => f.write_str("ClearClipHistory"),
            Self::Subscribe => f.write_str("Subscribe"),
            Self::Ping(d) => f.debug_tuple("Ping").field(d).finish(),
            Self::SendClipboard(d) => f.debug_tuple("SendClipboard").field(d).finish(),
            Self::SendFile(d, _) => f.debug_tuple("SendFile").field(d).finish_non_exhaustive(),
            Self::Handoff(d, item) => f.debug_tuple("Handoff").field(d).field(item).finish(),
            Self::Ring(d) => f.debug_tuple("Ring").field(d).finish(),
            Self::StopRing(d) => f.debug_tuple("StopRing").field(d).finish(),
            Self::Answer(ask, answer) => f.debug_tuple("Answer").field(ask).field(answer).finish(),
            Self::SetFeature(d, feature, state) => f
                .debug_tuple("SetFeature")
                .field(d)
                .field(feature)
                .field(state)
                .finish(),
        }
    }
}

/// The supervisor's answer to a [`ControlRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlResponse {
    /// To [`ControlRequest::Status`].
    Status(Vec<LinkedPeer>),
    /// To [`ControlRequest::Ping`]: the round trip in milliseconds.
    Pong(u64),
    /// To [`ControlRequest::ClipHistory`], newest first.
    ClipHistory(Vec<ClipItem>),
    /// The request succeeded.
    Done,
    /// The request failed; the error's message.
    Failed(String),
}

/// Why the control socket failed ([`crate::CoreError::Control`]).
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    /// A message is over its size limit.
    #[error("message over the size limit")]
    TooLarge,
    /// A message could not be decoded.
    #[error("unreadable message: {0}")]
    Decode(#[from] ciborium::de::Error<io::Error>),
    /// A message could not be encoded.
    #[error("cannot encode message: {0}")]
    Encode(#[from] ciborium::ser::Error<io::Error>),
    /// The supervisor closed the connection without answering.
    #[error("the supervisor closed the connection")]
    Closed,
    /// The supervisor answered with another kind of response.
    #[error("the supervisor gave an unexpected answer")]
    Unexpected,
    /// The supervisor (in another process) refused the request.
    #[error("{0}")]
    Failed(String),
    /// A file to send is not an absolute path to a regular file.
    #[error("not an absolute path to a regular file")]
    NotAFile,
}

/// Write `message`, refusing one over `cap` bytes.
pub(super) fn write_message(w: &mut impl Write, message: &impl Serialize, cap: u32) -> Result<()> {
    let mut body = Vec::new();
    ciborium::into_writer(message, &mut body).map_err(ControlError::from)?;
    let len = u32::try_from(body.len())
        .ok()
        .filter(|&n| n <= cap)
        .ok_or(ControlError::TooLarge)?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(&body)?;
    Ok(w.flush()?)
}

/// The next message (at most `cap` bytes); `None` when the other side hung
/// up between messages.
pub(super) fn read_message<T: DeserializeOwned>(r: &mut impl Read, cap: u32) -> Result<Option<T>> {
    let mut len = [0; 4];
    match r.read_exact(&mut len) {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        read => read?,
    }
    let len = u32::from_be_bytes(len);
    if len > cap {
        return Err(ControlError::TooLarge.into());
    }
    let mut body = vec![0; len as usize];
    r.read_exact(&mut body)?;
    Ok(Some(
        ciborium::from_reader(body.as_slice()).map_err(ControlError::from)?,
    ))
}
