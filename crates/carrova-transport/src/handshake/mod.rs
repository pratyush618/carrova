//! PAKE-inside-QUIC handshake (pairing, resume and link).
//!
//! On the first bidirectional stream, opened by the receiver:
//!
//! ```text
//! R → S  Hello { proto, Pair { nameplate } | Resume { nonce, tag } | Link { nonce, tag } }
//! S → R  Welcome | Reject(reason)
//! pair:   R → S  Pake(B)          S → R  Pake(A), Confirm(sender)
//! resume, link:                   S → R  Confirm(sender proof)
//! R → S  Confirm(receiver)      (only after the sender's verified)
//! pair, both remembering devices (`Hello.identify`, sender has a key),
//! and always on a link (the expected peer's key, device-link signature):
//!         S → R  Identify(sender)  R → S  Identify(receiver)
//! S → R  Established            (only after the receiver's verified)
//! pair-only (`Hello.purpose`, always identifying; ADR 0002 §1):
//!         S → R  PeerInfo  R → S  PeerInfo
//!         S → R  Linked    (once the sender stored the peer)
//! ```
//!
//! A pair-only receiver holding a pinned key (scanned QR code) checks the
//! sender's `Identify` against it before revealing its own.
//!
//! Identity proofs ([`PeerIdentity`]) are signed over the channel binding
//! below; a bad one fails the pairing. Older peers neither ask nor answer,
//! so they pair as before.
//!
//! A resume `Hello` tag uses [`TagPurpose::Hello`], so a public discovery
//! tag cannot stand in for it, and each nonce is admitted once per
//! listener ([`ResumeNonces`]). A link `Hello` (ADR 0002 §3) carries no
//! key: the listener checks its tag against every known peer's secret,
//! admits a recognised nonce once (same set), and closes any failed link
//! with `LinkFailed`, telling the peer nothing more.
//!
//! Confirmations are bound to this TLS session (exporter) and both
//! certificates ([`ChannelBinding`]). An [`Authenticated`] value exists only
//! after both checks passed; it is the only way to build a
//! [`crate::Connection`].
//!
//! [`TagPurpose::Hello`]: carrova_crypto::pairing::TagPurpose::Hello
//! [`ResumeNonces`]: crate::replay::ResumeNonces
//! [`ChannelBinding`]: carrova_crypto::pairing::ChannelBinding

use carrova_crypto::pairing::{CertHash, ResumeSecret};
use quinn::SendStream;

use crate::wire::StreamReader;

mod identify;
mod io;
mod keys;
mod pair_only;
mod receiver;
mod secret;
mod sender;

pub use identify::PeerIdentity;
pub(crate) use keys::binding;
pub use pair_only::{PairedPeer, PeerInfo, PendingPair};
pub(crate) use pair_only::{offer, receive};
pub(crate) use receiver::connect;
pub(crate) use secret::{Expected, Secret};
pub(crate) use sender::{AttemptFailure, sender_attempt};

/// A connection on which both sides proved the shared secret.
#[derive(Debug)]
pub(crate) struct Authenticated {
    pub(crate) conn: quinn::Connection,
    pub(crate) send: SendStream,
    pub(crate) recv: StreamReader,
    pub(crate) peer_cert: CertHash,
    /// The session's resume secret (none on a link).
    pub(crate) resume: Option<ResumeSecret>,
    /// Code pairing where both sides proved their device identity.
    pub(crate) peer_identity: Option<PeerIdentity>,
}
