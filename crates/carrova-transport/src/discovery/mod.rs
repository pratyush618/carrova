//! LAN discovery: mDNS-SD plus a UDP broadcast fallback (IPv4 only).
//!
//! The sender advertises the nameplate of its current code (never the
//! words) or a resume tag that rotates with every advertisement:
//!
//! - mDNS: service [`SERVICE_TYPE`], TXT [`TXT_VERSION`] plus
//!   [`TXT_NAMEPLATE`](crate::TXT_NAMEPLATE) (and
//!   [`TXT_DEVICE_NAME`], if known) or
//!   [`TXT_RESUME_TAG`](crate::TXT_RESUME_TAG), the QUIC port in SRV. The
//!   mdns-sd daemon runs on its own thread.
//! - Broadcast: the receiver sends `BROADCAST_QUERY ‖ key=value` to
//!   [`BROADCAST_PORT`]; a sender with that offer answers
//!   `BROADCAST_REPLY ‖ <quic port>`. A sender offering a code answers
//!   [`BROADCAST_BROWSE`] with `BROADCAST_REPLY ‖ <quic port> ‖ ' ' ‖
//!   <device name>` ([`browse`]).
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

mod browse;
mod interfaces;
mod seek;

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, Mutex, PoisonError};

use carrova_platform::Platform;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

pub use self::browse::{Nearby, browse};
pub(crate) use self::interfaces::primary_lan_ipv4;
use self::interfaces::{broadcast_targets, plausible_sender};
use self::seek::Seek;
pub(crate) use self::seek::{Offer, Subject, TagProof};
use crate::error::Result;
use crate::runtime::runtime;
use crate::tuning::{BROADCAST_MAX_DATAGRAM, BROADCAST_RETRY, MDNS_POLL};
use crate::{
    BROADCAST_BROWSE, BROADCAST_PORT, BROADCAST_REPLY, PROTOCOL_VERSION, SERVICE_TYPE,
    TXT_DEVICE_NAME, TXT_VERSION,
};

/// What a discovery answer yields, from a plausible sender `addr` that
/// announced `name` (or none).
type Make<T> = fn(SocketAddrV4, Option<String>) -> T;

fn broadcast_socket(platform: &dyn Platform, port: u16) -> Result<tokio::net::UdpSocket> {
    let socket = platform.bind_broadcast_udp(port)?;
    let _guard = runtime()?.enter();
    Ok(tokio::net::UdpSocket::from_std(socket)?)
}

// ------------------------------------------------------------------ sender

/// The sender's advertisement: mDNS registration plus broadcast responder.
/// Failing parts are logged and skipped (a fallback ticket still works).
/// Everything stops on drop.
pub(crate) struct Advertiser {
    port: u16,
    /// mDNS instance and host label (unique per listener).
    instance: String,
    /// Device name announced while a code is offered ([`txt_fit`]).
    name: Option<String>,
    current: Arc<Mutex<Option<Offer>>>,
    mdns: Option<ServiceDaemon>,
    registered: Option<String>,
    responder: Option<JoinHandle<()>>,
}

impl Advertiser {
    /// Start the responders for a QUIC endpoint on `port`, naming the
    /// sender `name` while it offers a code.
    pub(crate) fn start(
        platform: &dyn Platform,
        port: u16,
        instance: String,
        name: Option<String>,
    ) -> Result<Self> {
        let name = name.map(|n| txt_fit(&n).to_owned());
        let current = Arc::new(Mutex::new(None));
        let responder = match broadcast_socket(platform, BROADCAST_PORT) {
            Ok(socket) => {
                Some(runtime()?.spawn(respond(socket, Arc::clone(&current), port, name.clone())))
            }
            Err(e) => {
                tracing::warn!(error = %e, port = BROADCAST_PORT, "broadcast discovery unavailable");
                None
            }
        };
        let mdns = ServiceDaemon::new()
            .inspect_err(|e| tracing::warn!(error = %e, "mDNS discovery unavailable"))
            .ok();
        Ok(Self {
            port,
            instance,
            name,
            current,
            mdns,
            registered: None,
            responder,
        })
    }

    /// Replace the offer.
    pub(crate) fn set(&mut self, offer: Offer) {
        let key = offer.advert().key().as_str();
        let value = offer.advert().value();
        let name = self.name.as_deref().filter(|_| offer.is_pairing());
        *self.current.lock().unwrap_or_else(PoisonError::into_inner) = Some(offer);
        let Some(daemon) = &self.mdns else { return };
        if let Some(old) = self.registered.take() {
            // Fire and forget: the goodbye is sent by the daemon thread.
            let _ = daemon.unregister(&old);
        }
        let version = PROTOCOL_VERSION.to_string();
        let mut txt = vec![(TXT_VERSION, version.as_str()), (key, value.as_str())];
        txt.extend(name.map(|n| (TXT_DEVICE_NAME, n)));
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &self.instance,
            &format!("{}.local.", self.instance),
            "",
            self.port,
            &txt[..],
        )
        .map(ServiceInfo::enable_addr_auto);
        match info.and_then(|info| {
            let name = info.get_fullname().to_owned();
            daemon.register(info).map(|()| name)
        }) {
            Ok(name) => self.registered = Some(name),
            Err(e) => tracing::warn!(error = %e, "mDNS advertisement failed"),
        }
    }
}

impl std::fmt::Debug for Advertiser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Advertiser")
            .field("port", &self.port)
            .field("instance", &self.instance)
            .field("mdns", &self.mdns.is_some())
            .field("registered", &self.registered)
            .finish_non_exhaustive()
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        if let Some(task) = self.responder.take() {
            task.abort();
        }
        if let Some(daemon) = self.mdns.take() {
            if let Some(name) = self.registered.take() {
                let _ = daemon.unregister(&name);
            }
            let _ = daemon.shutdown();
        }
    }
}

/// Longest device name in one TXT entry (`dn=<name>`, at most 255 bytes),
/// cut on a char boundary.
fn txt_fit(name: &str) -> &str {
    let mut end = name
        .len()
        .min(usize::from(u8::MAX) - TXT_DEVICE_NAME.len() - 1);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

/// The reply to broadcast `query` while `offer` is current: the port for
/// a query naming the offer; port and `name` for a browse query while a
/// code is offered; else none.
fn reply(offer: &Offer, query: &[u8], port: u16, name: Option<&str>) -> Option<Vec<u8>> {
    let mut reply = BROADCAST_REPLY.to_vec();
    reply.extend_from_slice(port.to_string().as_bytes());
    if offer.answers(query) {
        return Some(reply);
    }
    let name = name.filter(|_| query == BROADCAST_BROWSE && offer.is_pairing())?;
    reply.push(b' ');
    reply.extend_from_slice(name.as_bytes());
    Some(reply)
}

/// Answer broadcast queries that ask for the current offer, and browse
/// queries while a code is offered.
async fn respond(
    socket: tokio::net::UdpSocket,
    current: Arc<Mutex<Option<Offer>>>,
    port: u16,
    name: Option<String>,
) {
    let mut buf = [0u8; BROADCAST_MAX_DATAGRAM];
    loop {
        let (n, from) = match socket.recv_from(&mut buf).await {
            Ok(x) => x,
            Err(e) => {
                tracing::debug!(error = %e, "broadcast receive failed");
                continue;
            }
        };
        let answer = current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .and_then(|offer| reply(offer, &buf[..n], port, name.as_deref()));
        if let Some(answer) = answer
            && let Err(e) = socket.send_to(&answer, from).await
        {
            tracing::debug!(error = %e, "broadcast reply failed");
        }
    }
}

// ------------------------------------------------------------------ receiver

/// Look for `wanted` until `deadline` or until the returned receiver is
/// dropped. Plausible candidate addresses arrive as found (duplicates
/// possible).
pub(crate) fn discover(
    platform: &Arc<dyn Platform>,
    wanted: Subject,
    deadline: Instant,
) -> Result<mpsc::Receiver<SocketAddrV4>> {
    let rt = runtime()?;
    let (tx, rx) = mpsc::channel(16);
    let platform = Arc::clone(platform);
    let query = wanted.fresh_seek()?.to_query();
    let broadcast_tx = tx.clone();
    rt.spawn(async move {
        broadcast_query(platform.as_ref(), &query, &broadcast_tx, deadline, |a, _| a).await;
    });
    rt.spawn_blocking(move || {
        mdns_browse(
            |s| advertises(s, &wanted),
            &tx,
            deadline.into_std(),
            |a, _| a,
        );
    });
    Ok(rx)
}

/// Send `query` to every broadcast target each [`BROADCAST_RETRY`] until
/// `deadline` (or `tx` closes), yielding each plausible answer.
async fn broadcast_query<T>(
    platform: &dyn Platform,
    query: &[u8],
    tx: &mpsc::Sender<T>,
    deadline: Instant,
    make: Make<T>,
) {
    let socket = match broadcast_socket(platform, 0) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(error = %e, "broadcast discovery unavailable");
            return;
        }
    };
    let targets = broadcast_targets(platform);
    let loopback_queried = targets.iter().any(Ipv4Addr::is_loopback);
    let mut buf = [0u8; BROADCAST_MAX_DATAGRAM];
    while Instant::now() < deadline && !tx.is_closed() {
        for ip in &targets {
            let target = SocketAddrV4::new(*ip, BROADCAST_PORT);
            if let Err(e) = socket.send_to(query, target).await {
                tracing::debug!(error = %e, %target, "broadcast query failed");
            }
        }
        let round_end = deadline.min(Instant::now() + BROADCAST_RETRY);
        while let Ok(Ok((n, from))) =
            tokio::time::timeout_at(round_end, socket.recv_from(&mut buf)).await
        {
            let (SocketAddr::V4(from), Some((port, name))) = (from, parse_reply(&buf[..n])) else {
                continue;
            };
            let candidate = SocketAddrV4::new(*from.ip(), port);
            if !plausible_sender(candidate, loopback_queried) {
                tracing::debug!(%candidate, "implausible broadcast answer ignored");
                continue;
            }
            if tx.send(make(candidate, name)).await.is_err() {
                return;
            }
        }
    }
}

/// A broadcast reply's port and, answering a browse, device name.
fn parse_reply(datagram: &[u8]) -> Option<(u16, Option<String>)> {
    let body = std::str::from_utf8(datagram.strip_prefix(BROADCAST_REPLY)?).ok()?;
    let (port, name) = match body.split_once(' ') {
        Some((port, name)) => (port, Some(name.to_owned())),
        None => (body, None),
    };
    Some((port.parse().ok()?, name))
}

/// Does a resolved mDNS service advertise `wanted` (current protocol
/// version, key under `wanted`'s TXT key)?
fn advertises(service: &mdns_sd::ResolvedService, wanted: &Subject) -> bool {
    let key = wanted.key();
    service.get_property_val_str(TXT_VERSION) == Some(PROTOCOL_VERSION.to_string().as_str())
        && service
            .get_property_val_str(key.as_str())
            .and_then(|value| Seek::parse(key, value))
            .is_some_and(|seek| wanted.recognises(&seek))
}

/// Browse mDNS until `deadline` (or `tx` closes), yielding each plausible
/// address of every service `matches` accepts, with its device name.
fn mdns_browse<T>(
    matches: impl Fn(&mdns_sd::ResolvedService) -> bool,
    tx: &mpsc::Sender<T>,
    deadline: std::time::Instant,
    make: Make<T>,
) {
    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            tracing::debug!(error = %e, "mDNS browse unavailable");
            return;
        }
    };
    let events = match daemon.browse(SERVICE_TYPE) {
        Ok(rx) => rx,
        Err(e) => {
            tracing::debug!(error = %e, "mDNS browse failed");
            let _ = daemon.shutdown();
            return;
        }
    };
    let mut seen = HashSet::new();
    'browse: while std::time::Instant::now() < deadline && !tx.is_closed() {
        let Ok(ServiceEvent::ServiceResolved(service)) = events.recv_timeout(MDNS_POLL) else {
            continue;
        };
        if !matches(&service) {
            continue;
        }
        let name = service
            .get_property_val_str(TXT_DEVICE_NAME)
            .map(str::to_owned);
        for ip in service.get_addresses_v4() {
            let addr = SocketAddrV4::new(ip, service.get_port());
            // mDNS queries go to multicast, never to loopback.
            if plausible_sender(addr, false)
                && seen.insert(addr)
                && tx.blocking_send(make(addr, name.clone())).is_err()
            {
                break 'browse;
            }
        }
    }
    let _ = daemon.shutdown();
}

#[cfg(test)]
#[path = "../../tests/discovery/unit.rs"]
mod tests;
