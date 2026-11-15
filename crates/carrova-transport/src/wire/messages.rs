//! CBOR control messages.

use carrova_crypto::identity::{DEVICE_PUBLIC_LEN, IDENTITY_SIG_LEN};
use carrova_crypto::pairing::{CONFIRMATION_LEN, LINK_TAG_LEN, RESUME_TAG_LEN, TAG_NONCE_LEN};
use carrova_manifest::ChunkHash;
use carrova_platform::OsKind;
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
        /// The receiver remembers devices: a code-paired sender that does
        /// too proves its identity ([`Handshake::Identify`]). Older
        /// receivers omit it (`false`).
        #[serde(default)]
        identify: bool,
        /// Whether the paired session carries a transfer. Older receivers
        /// omit it ([`PairPurpose::Move`]).
        #[serde(default)]
        purpose: PairPurpose,
    },
    /// Sender → receiver: the intent is being offered; continue.
    Welcome,
    /// Sender → receiver: not offered; the connection closes.
    Reject(RejectReason),
    /// One SPAKE2 message.
    Pake(#[serde(with = "serde_bytes")] Vec<u8>),
    /// Key confirmation or resume proof.
    Confirm(#[serde(with = "serde_bytes")] [u8; CONFIRMATION_LEN]),
    /// Device identity proof (code pairing only, after both
    /// confirmations): the sender's if the `Hello` asked for it, then the
    /// receiver's.
    Identify {
        /// [`carrova_crypto::identity::DevicePublic`] bytes.
        #[serde(with = "serde_bytes")]
        public: [u8; DEVICE_PUBLIC_LEN],
        /// Signature over this connection's channel binding.
        #[serde(with = "serde_bytes")]
        sig: [u8; IDENTITY_SIG_LEN],
    },
    /// Sender → receiver: both checks passed.
    Established,
    /// Link handshake, sender → receiver: the link tag verified; the
    /// connection is authenticated without a transfer.
    Linked,
    /// Pair-only handshake: a peer's display name and OS, sent by each
    /// side after `Established`.
    PeerInfo {
        /// The peer's device name.
        name: String,
        /// The peer's OS family.
        os: OsKind,
    },
}

/// What a code-paired session is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum PairPurpose {
    /// Pair, then move data.
    #[default]
    Move,
    /// Pair to remember each other; no transfer follows.
    PairOnly,
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
    /// Connect to a linked device: `tag` proves the link secret for `nonce`.
    Link {
        /// A fresh nonce per attempt.
        #[serde(with = "serde_bytes")]
        nonce: [u8; TAG_NONCE_LEN],
        /// [`carrova_crypto::pairing::LinkTag`] bytes.
        #[serde(with = "serde_bytes")]
        tag: [u8; LINK_TAG_LEN],
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

/// First frame on every stream after the control stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum StreamOpen {
    /// A chunk stream: [`ChunkRequest`] frames follow.
    Chunks,
    /// The receiver finished fetching; the sender finishes this stream once
    /// every chunk stream is drained.
    FetchDone,
    /// A lane ([`crate::Stream`]), opened by either side; only
    /// [`crate::Connection::accept_stream`] takes it.
    Lane,
}

/// A batch of chunks to send, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChunkRequest {
    pub(crate) hashes: Vec<ChunkHash>,
}

#[cfg(test)]
#[path = "../../tests/wire/messages/unit.rs"]
mod tests;
