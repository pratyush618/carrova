//! CBOR control messages.

use carrova_crypto::pairing::{CONFIRMATION_LEN, RESUME_TAG_LEN, TAG_NONCE_LEN};
use carrova_manifest::ChunkHash;
use serde::{Deserialize, Serialize};

/// Handshake messages on the control stream (before a `Connection` exists).
#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Handshake {
    /// Receiver → sender: first message.
    Hello {
        /// [`crate::PROTOCOL_VERSION`].
        proto: u32,
        /// What the receiver wants.
        intent: Intent,
    },
    /// Sender → receiver: the intent is being offered; continue.
    Welcome,
    /// Sender → receiver: not offered; the connection closes.
    Reject(RejectReason),
    /// One SPAKE2 message.
    Pake(#[serde(with = "serde_bytes")] Vec<u8>),
    /// Key confirmation or resume proof.
    Confirm(#[serde(with = "serde_bytes")] [u8; CONFIRMATION_LEN]),
    /// Sender → receiver: both checks passed.
    Established,
}

/// What a receiver's `Hello` asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Intent {
    /// Pair with the code on this nameplate.
    Pair {
        /// Public part of the code.
        nameplate: u16,
    },
    /// Reconnect the session whose secret makes `tag` for `nonce`
    /// (a fresh nonce per attempt, so attempts are unlinkable).
    Resume {
        /// [`carrova_crypto::pairing::TagNonce`] bytes.
        #[serde(with = "serde_bytes")]
        nonce: [u8; TAG_NONCE_LEN],
        /// [`carrova_crypto::pairing::ResumeTag`] bytes.
        #[serde(with = "serde_bytes")]
        tag: [u8; RESUME_TAG_LEN],
    },
}

/// Why the sender rejected a `Hello`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RejectReason {
    /// Different protocol version.
    Version,
    /// Nameplate or resume tag not offered.
    NotOffered,
}

/// First frame on every receiver-opened stream after the control stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum StreamOpen {
    /// A chunk stream: [`ChunkRequest`] frames follow.
    Chunks,
    /// The receiver finished fetching; the sender finishes this stream once
    /// every chunk stream is drained.
    FetchDone,
}

/// A batch of chunks to send, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChunkRequest {
    pub(crate) hashes: Vec<ChunkHash>,
}
