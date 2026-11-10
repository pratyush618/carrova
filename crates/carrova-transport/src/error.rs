//! Transport error type and the mapping from quinn's errors.

use carrova_crypto::pairing::PairingError;
use carrova_manifest::{ChunkHash, MAX_CHUNK_LEN};
use quinn::{ConnectionError, ReadError, ReadExactError, WriteError};

use crate::connection::FetchAbort;
use crate::tuning::{CLOSE_LINGER, SERVED_BYTES_FACTOR};
use crate::wire::CloseCode;

/// Errors from the transport. Never contain codes or key material.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    /// Pairing failed: wrong code or tampered connection. The sender burns
    /// its code after this.
    #[error("pairing failed: wrong code or tampered connection")]
    PairingFailed,
    /// No answer within the caller's timeout.
    #[error("timed out")]
    Timeout,
    /// The peer disconnected or became unreachable.
    #[error("connection to peer lost")]
    PeerLost,
    /// The caller's [`crate::CancelSignal`] stopped the call.
    #[error("cancelled")]
    Cancelled,
    /// The peer cancelled the session ([`crate::Connection::close_cancelled`]).
    #[error("the other machine cancelled the session")]
    PeerCancelled,
    /// Discovery found no sender for the nameplate or resume tag.
    #[error("no matching sender found on the local network")]
    NotFound,
    /// The peer broke Carrova's wire protocol (detected by the transport).
    #[error("protocol error: {0}")]
    Violation(ProtocolViolation),
    /// A frame exceeds [`crate::MAX_FRAME_LEN`].
    #[error("frame of {len} bytes exceeds the limit")]
    FrameTooLarge {
        /// Announced or encoded body length.
        len: usize,
    },
    /// A typed ticket is malformed.
    #[error("invalid ticket: {0}")]
    InvalidTicket(&'static str),
    /// The fetch callback stopped the transfer.
    #[error(transparent)]
    Aborted(#[from] FetchAbort),
    /// Pairing-code or key-schedule error.
    #[error(transparent)]
    Pairing(#[from] PairingError),
    /// Socket or runtime I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Local transport setup failed (TLS configuration, certificate
    /// generation, an internal task). Not caused by the peer.
    #[error("transport setup failed: {0}")]
    Internal(SetupFailure),
}

/// How the peer broke the wire protocol. The text is part of
/// [`TransportError`]'s message (not a separate cause).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolViolation {
    /// The peer closed the connection with the protocol-violation code.
    #[error("peer closed the connection: protocol violation")]
    PeerClosed,
    /// The peer reset a stream we were reading.
    #[error("peer reset the stream")]
    StreamReset,
    /// The peer stopped a stream we were writing.
    #[error("peer stopped the stream")]
    StreamStopped,
    /// A stream closed where it must not.
    #[error("stream unexpectedly closed")]
    StreamClosed,
    /// A stream ended inside a message.
    #[error("stream ended mid-message")]
    TruncatedMessage,
    /// A control frame did not decode (decoder detail).
    #[error("malformed message: {0}")]
    Malformed(String),
    /// A control message could not be encoded (encoder detail).
    #[error("encoding a message failed: {0}")]
    Unencodable(String),
    /// A handshake message arrived out of order.
    #[error("unexpected handshake message (wanted {wanted})")]
    UnexpectedHandshake {
        /// The message the handshake expected.
        wanted: &'static str,
    },
    /// The sender rejected our protocol version.
    #[error("the sender runs an incompatible Carrova version")]
    IncompatibleVersion,
    /// The TLS peer presented no certificate.
    #[error("peer presented no certificate")]
    NoPeerCertificate,
    /// A chunk record header has an unknown kind byte.
    #[error("unknown chunk record kind")]
    UnknownRecordKind,
    /// A chunk is longer than [`MAX_CHUNK_LEN`].
    #[error("chunk of {len} bytes exceeds the {MAX_CHUNK_LEN}-byte limit")]
    ChunkTooLong {
        /// Announced length.
        len: u32,
    },
    /// A chunk length does not fit this platform's `usize`.
    #[error("chunk length overflow")]
    ChunkLengthOverflow,
    /// A chunk stream ended with requested chunks outstanding.
    #[error("chunk stream ended before all requested chunks")]
    ChunkStreamEnded,
    /// The sender answered out of request order.
    #[error("sender answered with a chunk that was not requested next")]
    UnrequestedChunk,
    /// A chunk record's length differs from the manifest's.
    #[error("chunk length differs from the manifest")]
    ChunkLengthMismatch,
    /// The fetch-done stream carried data.
    #[error("unexpected data on the fetch-done stream")]
    FetchDoneData,
    /// The receiver ended the fetch phase twice.
    #[error("fetch ended twice")]
    FetchEndedTwice,
    /// The receiver requested a chunk outside the manifest.
    #[error("receiver requested chunk {0}, which is not in the manifest")]
    NotInManifest(ChunkHash),
    /// The receiver requested more than the served-bytes ceiling.
    #[error("receiver requested more than {SERVED_BYTES_FACTOR}× the manifest's chunk bytes")]
    OverRequested,
}

/// Why local transport setup failed. The text is part of
/// [`TransportError`]'s message (not a separate cause).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetupFailure {
    /// A setup step failed with a library error (its text).
    #[error("{step}: {detail}")]
    Step {
        /// The step, e.g. `"TLS client config"`.
        step: &'static str,
        /// The library error's text.
        detail: String,
    },
    /// The TLS session offers no keying-material exporter.
    #[error("TLS exporter unavailable")]
    ExporterUnavailable,
    /// The handshake reached a state its inputs rule out.
    #[error("handshake state mismatch")]
    HandshakeStateMismatch,
    /// The QUIC endpoint closed while accepting.
    #[error("endpoint closed")]
    EndpointClosed,
    /// The QUIC endpoint is bound to IPv6.
    #[error("endpoint is not bound to IPv4")]
    NotIpv4,
    /// Serving was stopped locally.
    #[error("serving was stopped")]
    ServingStopped,
    /// The connection driver task was joined twice.
    #[error("driver already joined")]
    DriverJoined,
}

impl From<ProtocolViolation> for TransportError {
    fn from(v: ProtocolViolation) -> Self {
        Self::Violation(v)
    }
}

impl From<SetupFailure> for TransportError {
    fn from(f: SetupFailure) -> Self {
        Self::Internal(f)
    }
}

/// Crate result alias.
pub type Result<T, E = TransportError> = std::result::Result<T, E>;

impl TransportError {
    /// The connection ended. A close by the peer with one of our codes keeps
    /// its meaning; anything else (timeout, reset, local close) is a lost
    /// peer.
    pub(crate) fn connection(err: &ConnectionError) -> Self {
        let ConnectionError::ApplicationClosed(close) = err else {
            return Self::PeerLost;
        };
        match CloseCode::from_wire(close.error_code) {
            Some(CloseCode::PairingFailed) => Self::PairingFailed,
            Some(CloseCode::Protocol) => ProtocolViolation::PeerClosed.into(),
            Some(CloseCode::Cancelled) => Self::PeerCancelled,
            Some(CloseCode::Normal | CloseCode::Rejected) | None => Self::PeerLost,
        }
    }

    /// Why the peer is going away, once it ended a stream or reset it:
    /// its close reason if the close arrives within [`CLOSE_LINGER`], else
    /// [`Self::PeerLost`].
    pub(crate) async fn closed_by_peer(conn: &quinn::Connection) -> Self {
        match tokio::time::timeout(CLOSE_LINGER, conn.closed()).await {
            Ok(reason) => Self::connection(&reason),
            Err(_) => Self::PeerLost,
        }
    }

    pub(crate) fn read(err: &ReadError) -> Self {
        match err {
            ReadError::ConnectionLost(e) => Self::connection(e),
            ReadError::Reset(_) => ProtocolViolation::StreamReset.into(),
            ReadError::ClosedStream
            | ReadError::ZeroRttRejected
            | ReadError::IllegalOrderedRead => ProtocolViolation::StreamClosed.into(),
        }
    }

    pub(crate) fn read_exact(err: &ReadExactError) -> Self {
        match err {
            ReadExactError::FinishedEarly(_) => ProtocolViolation::TruncatedMessage.into(),
            ReadExactError::ReadError(e) => Self::read(e),
        }
    }

    pub(crate) fn write(err: &WriteError) -> Self {
        match err {
            WriteError::ConnectionLost(e) => Self::connection(e),
            WriteError::Stopped(_) => ProtocolViolation::StreamStopped.into(),
            WriteError::ClosedStream | WriteError::ZeroRttRejected => {
                ProtocolViolation::StreamClosed.into()
            }
        }
    }

    /// Setup `step` failed with `err`.
    pub(crate) fn internal(step: &'static str, err: impl std::fmt::Display) -> Self {
        SetupFailure::Step {
            step,
            detail: err.to_string(),
        }
        .into()
    }

    /// Whether the connection is gone (retry or resume), not "the peer
    /// misbehaved": the peer was lost, timed out, or the socket failed.
    #[must_use]
    pub const fn is_lost(&self) -> bool {
        matches!(self, Self::PeerLost | Self::Timeout | Self::Io(_))
    }

    /// The peer broke the wire protocol (the connection should be closed
    /// with [`CloseCode::Protocol`]).
    pub(crate) const fn is_violation(&self) -> bool {
        matches!(self, Self::Violation(_) | Self::FrameTooLarge { .. })
    }
}

#[cfg(test)]
#[path = "../tests/error/unit.rs"]
mod tests;
