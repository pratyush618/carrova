//! Carrova LAN transport: discovery, PAKE-authenticated QUIC connections and
//! chunk transfer.
//!
//! The API is **blocking**; a tokio runtime and quinn live inside this
//! crate only. Callers (the synchronous `carrova-core` engine) never see
//! async code.
//!
//! - **Discovery:** the sender advertises `_carrova._udp.local.` over mDNS
//!   ([`SERVICE_TYPE`], TXT [`TXT_VERSION`] plus [`TXT_NAMEPLATE`] or
//!   [`TXT_RESUME_TAG`]) and answers UDP broadcast queries
//!   ([`BROADCAST_QUERY`] on [`BROADCAST_PORT`]). IPv4 only. When discovery
//!   fails the user types a [`Ticket`] (`<code>@<ipv4>:<port>`). While a
//!   code is offered it also names itself ([`TXT_DEVICE_NAME`], answers to
//!   [`BROADCAST_BROWSE`]) so receivers can [`browse`] senders nearby.
//! - **Handshake:** throwaway self-signed certificates, mutual TLS 1.3 with
//!   ALPN [`ALPN`]; the receiver opens the first bidirectional stream and
//!   runs SPAKE2 + key confirmation from [`carrova_crypto::pairing`] (or the
//!   resume proof). Only then does a [`Connection`] exist, so no
//!   application traffic is possible before authentication.
//! - **Frames:** control messages are a u32 big-endian length + CBOR body,
//!   at most [`MAX_FRAME_LEN`] bytes. Chunk data uses a fixed binary record
//!   header plus raw bytes (no CBOR on the bulk path).
//! - **Cancellation:** blocking calls watch the caller's [`CancelSignal`]
//!   (attached through the connect / listen options) and end with
//!   [`TransportError::Cancelled`]. [`Connection::close_cancelled`] tells
//!   the peer, whose calls end with [`TransportError::PeerCancelled`].
//! - **Chunks:** the receiver pulls ([`Connection::fetch_chunks`]) over
//!   parallel streams; the sender serves only chunks in its manifest
//!   ([`Connection::serve_chunks`]). Every chunk is BLAKE3-verified on
//!   arrival and re-requested up to [`MAX_CHUNK_RETRIES`] times.

mod accept;
mod admission;
mod cancel;
mod claim;
mod connect;
mod connection;
mod discovery;
mod endpoint;
mod error;
mod fetch;
mod handshake;
mod listener;
mod race;
mod replay;
mod runtime;
mod serve;
#[cfg(test)]
#[path = "../tests/lib/unit.rs"]
mod tests;
mod ticket;
mod tls;
mod tuning;
mod wire;

pub use cancel::CancelSignal;
pub use connect::{ConnectOptions, Target, connect};
pub use connection::{
    AllowedChunks, ChunkFailure, Connection, FailedChunk, FetchAbort, FetchReport, OnChunk,
    ServeEnd,
};
pub use discovery::{Nearby, browse};
pub use error::{ProtocolViolation, Result, SetupFailure, TransportError};
pub use handshake::PeerIdentity;
pub use listener::{ListenOptions, Listener};
pub use ticket::Ticket;

/// QUIC/TLS application protocol identifier.
pub const ALPN: &[u8] = b"carrova/1";
/// Wire protocol version (TXT [`TXT_VERSION`]).
pub const PROTOCOL_VERSION: u32 = 1;
/// mDNS service type the sender advertises.
pub const SERVICE_TYPE: &str = "_carrova._udp.local.";
/// TXT key: protocol version.
pub const TXT_VERSION: &str = "v";
/// TXT key: nameplate of the code being offered (pairing).
pub const TXT_NAMEPLATE: &str = "np";
/// TXT key: resume tag of the session waiting for a reconnect, as
/// `<nonce hex><tag hex>` (fresh nonce per advertisement; see
/// [`carrova_crypto::pairing::ResumeSecret::tag_for`]).
pub const TXT_RESUME_TAG: &str = "rt";
/// TXT key: the sender's device name, present only while a code is
/// offered (never beside a resume tag). Peer-provided: sanitise before
/// display.
pub const TXT_DEVICE_NAME: &str = "dn";
/// UDP port of the broadcast discovery fallback.
pub const BROADCAST_PORT: u16 = 47821;
/// Broadcast discovery query prefix, followed by `np=<nameplate>` or
/// `rt=<nonce hex><resume tag hex>` (fresh nonce per query); a matching
/// sender replies with [`BROADCAST_REPLY`].
pub const BROADCAST_QUERY: &[u8] = b"CARROVA/1?";
/// Broadcast browse query: every sender offering a code and knowing its
/// device name replies [`BROADCAST_REPLY`] ‖ port ‖ `' '` ‖ name.
pub const BROADCAST_BROWSE: &[u8] = b"CARROVA/1?browse";
/// Broadcast discovery reply prefix, followed by the sender's QUIC port in
/// decimal (and, answering [`BROADCAST_BROWSE`], a space and its device
/// name); the reply's source address is the sender's IP.
pub const BROADCAST_REPLY: &[u8] = b"CARROVA/1!";
/// Largest frame body accepted or sent (1 MiB).
pub const MAX_FRAME_LEN: usize = 1024 * 1024;
/// Default number of parallel chunk streams.
pub const DEFAULT_PARALLEL_FETCHES: usize = 8;
/// Re-requests of a chunk whose bytes fail verification before it is
/// reported as failed.
pub const MAX_CHUNK_RETRIES: u32 = 3;
