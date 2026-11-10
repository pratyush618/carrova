//! PAKE-inside-QUIC handshake (pairing and resume).
//!
//! On the first bidirectional stream, opened by the receiver:
//!
//! ```text
//! R → S  Hello { proto, Pair { nameplate } | Resume { nonce, tag } }
//! S → R  Welcome | Reject(reason)
//! pair:   R → S  Pake(B)          S → R  Pake(A), Confirm(sender)
//! resume:                         S → R  Confirm(sender proof)
//! R → S  Confirm(receiver)      (only after the sender's verified)
//! S → R  Established            (only after the receiver's verified)
//! ```
//!
//! A resume `Hello` tag uses [`TagPurpose::Hello`], so a public discovery
//! tag cannot stand in for it, and each nonce is admitted once per
//! listener ([`ResumeNonces`]).
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

mod io;
mod keys;
mod receiver;
mod secret;
mod sender;

pub(crate) use keys::binding;
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
    pub(crate) resume: ResumeSecret,
}
