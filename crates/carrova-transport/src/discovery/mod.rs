//! LAN discovery: mDNS-SD plus a UDP broadcast fallback (IPv4 only).
//!
//! The sender advertises the nameplate of its current code (never the
//! words) or a resume tag that rotates with every advertisement:
//!
//! - mDNS: service [`SERVICE_TYPE`](crate::SERVICE_TYPE), TXT [`TXT_VERSION`](crate::TXT_VERSION) plus
//!   [`TXT_NAMEPLATE`](crate::TXT_NAMEPLATE) (and
//!   [`TXT_DEVICE_NAME`](crate::TXT_DEVICE_NAME), if known) or
//!   [`TXT_RESUME_TAG`](crate::TXT_RESUME_TAG), the QUIC port in SRV. The
//!   mdns-sd daemon runs on its own thread.
//! - Broadcast: the receiver sends `BROADCAST_QUERY ‖ key=value` to
//!   [`BROADCAST_PORT`](crate::BROADCAST_PORT); a sender with that offer answers
//!   `BROADCAST_REPLY ‖ <quic port>`. A sender offering a code answers
//!   [`BROADCAST_BROWSE`](crate::BROADCAST_BROWSE) with `BROADCAST_REPLY ‖ <quic port> ‖ ' ' ‖
//!   <device name>` ([`browse()`]).
//!
//! A presence ([`PresenceBeacon`]) advertises TXT [`TXT_VERSION`](crate::TXT_VERSION) plus
//! [`TXT_KNOWN_TAGS`](crate::TXT_KNOWN_TAGS) and, shown to everyone,
//! [`TXT_DEVICE_NAME`](crate::TXT_DEVICE_NAME) and [`TXT_OS`](crate::TXT_OS);
//! over broadcast it answers browse queries with the same entries.
//!
//! Older peers ignore what they do not know: an unknown TXT key, a browse
//! query (no `key=value`), a reply they never asked for.
//!
//! A resume value is `<nonce hex><tag hex>` with `tag =
//! ResumeSecret::tag_for(TagPurpose::Discovery, nonce)` and a fresh random nonce per sender
//! advertisement and per receiver query, so a session's adverts cannot be
//! linked without the secret; either side recognises the other by
//! recomputing the tag ([`seek`]).
//!
//! Answers are untrusted: the receiver drops implausible ones
//! ([`interfaces::plausible_sender`]) and races full handshakes to the
//! rest, so a spoofed answer can at worst cost one failed dial.

mod advertise;
mod browse;
mod interfaces;
mod presence;
mod query;
mod seek;

use carrova_platform::Platform;

pub(crate) use self::advertise::Advertiser;
pub use self::browse::{Nearby, browse};
pub(crate) use self::interfaces::primary_lan_ipv4;
pub use self::presence::{KnownTags, MAX_KNOWN_TAGS, PresenceAdvert, PresenceBeacon};
pub(crate) use self::query::discover;
pub(crate) use self::seek::{Offer, Subject, TagProof};
use crate::error::Result;
use crate::runtime::runtime;

fn broadcast_socket(platform: &dyn Platform, port: u16) -> Result<tokio::net::UdpSocket> {
    let socket = platform.bind_broadcast_udp(port)?;
    let _guard = runtime()?.enter();
    Ok(tokio::net::UdpSocket::from_std(socket)?)
}
