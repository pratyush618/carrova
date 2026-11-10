//! LAN discovery: mDNS-SD plus a UDP broadcast fallback (IPv4 only).
//!
//! The sender advertises the nameplate of its current code (never the
//! words) or a resume tag that rotates with every advertisement:
//!
//! - mDNS: service [`SERVICE_TYPE`], TXT [`TXT_VERSION`] plus
//!   [`TXT_NAMEPLATE`](crate::TXT_NAMEPLATE) or
//!   [`TXT_RESUME_TAG`](crate::TXT_RESUME_TAG), the QUIC port in SRV. The
//!   mdns-sd daemon runs on its own thread.
//! - Broadcast: the receiver sends `BROADCAST_QUERY ‖ key=value` to
//!   [`BROADCAST_PORT`]; a sender with that offer answers
//!   `BROADCAST_REPLY ‖ <quic port>`.
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

pub(crate) use self::interfaces::primary_lan_ipv4;
use self::interfaces::{broadcast_targets, plausible_sender};
use self::seek::Seek;
pub(crate) use self::seek::{Offer, Subject, TagProof};
use crate::error::Result;
use crate::runtime::runtime;
use crate::tuning::{BROADCAST_MAX_DATAGRAM, BROADCAST_RETRY, MDNS_POLL};
use crate::{BROADCAST_PORT, BROADCAST_REPLY, PROTOCOL_VERSION, SERVICE_TYPE, TXT_VERSION};

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
    current: Arc<Mutex<Option<Offer>>>,
    mdns: Option<ServiceDaemon>,
    registered: Option<String>,
    responder: Option<JoinHandle<()>>,
}

impl Advertiser {
    /// Start the responders for a QUIC endpoint on `port`.
    pub(crate) fn start(platform: &dyn Platform, port: u16, instance: String) -> Result<Self> {
        let current = Arc::new(Mutex::new(None));
        let responder = match broadcast_socket(platform, BROADCAST_PORT) {
            Ok(socket) => Some(runtime()?.spawn(respond(socket, Arc::clone(&current), port))),
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
        *self.current.lock().unwrap_or_else(PoisonError::into_inner) = Some(offer);
        let Some(daemon) = &self.mdns else { return };
        if let Some(old) = self.registered.take() {
            // Fire and forget: the goodbye is sent by the daemon thread.
            let _ = daemon.unregister(&old);
        }
        let version = PROTOCOL_VERSION.to_string();
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &self.instance,
            &format!("{}.local.", self.instance),
            "",
            self.port,
            &[(TXT_VERSION, version.as_str()), (key, value.as_str())][..],
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

/// Answer broadcast queries that ask for the current offer.
async fn respond(socket: tokio::net::UdpSocket, current: Arc<Mutex<Option<Offer>>>, port: u16) {
    let mut reply = BROADCAST_REPLY.to_vec();
    reply.extend_from_slice(port.to_string().as_bytes());
    let mut buf = [0u8; BROADCAST_MAX_DATAGRAM];
    loop {
        let (n, from) = match socket.recv_from(&mut buf).await {
            Ok(x) => x,
            Err(e) => {
                tracing::debug!(error = %e, "broadcast receive failed");
                continue;
            }
        };
        let answers = current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|offer| offer.answers(&buf[..n]));
        if answers && let Err(e) = socket.send_to(&reply, from).await {
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
    rt.spawn(broadcast_query(
        Arc::clone(platform),
        wanted.fresh_seek()?.to_query(),
        tx.clone(),
        deadline,
    ));
    rt.spawn_blocking(move || mdns_browse(&wanted, &tx, deadline.into_std()));
    Ok(rx)
}

async fn broadcast_query(
    platform: Arc<dyn Platform>,
    query: Vec<u8>,
    tx: mpsc::Sender<SocketAddrV4>,
    deadline: Instant,
) {
    let socket = match broadcast_socket(platform.as_ref(), 0) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(error = %e, "broadcast discovery unavailable");
            return;
        }
    };
    let targets = broadcast_targets(platform.as_ref());
    let loopback_queried = targets.iter().any(Ipv4Addr::is_loopback);
    let mut buf = [0u8; BROADCAST_MAX_DATAGRAM];
    while Instant::now() < deadline && !tx.is_closed() {
        for ip in &targets {
            let target = SocketAddrV4::new(*ip, BROADCAST_PORT);
            if let Err(e) = socket.send_to(&query, target).await {
                tracing::debug!(error = %e, %target, "broadcast query failed");
            }
        }
        let round_end = deadline.min(Instant::now() + BROADCAST_RETRY);
        while let Ok(Ok((n, from))) =
            tokio::time::timeout_at(round_end, socket.recv_from(&mut buf)).await
        {
            let (SocketAddr::V4(from), Some(port)) = (from, parse_reply(&buf[..n])) else {
                continue;
            };
            let candidate = SocketAddrV4::new(*from.ip(), port);
            if !plausible_sender(candidate, loopback_queried) {
                tracing::debug!(%candidate, "implausible broadcast answer ignored");
                continue;
            }
            if tx.send(candidate).await.is_err() {
                return;
            }
        }
    }
}

fn parse_reply(datagram: &[u8]) -> Option<u16> {
    let port = datagram.strip_prefix(BROADCAST_REPLY)?;
    std::str::from_utf8(port).ok()?.parse().ok()
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

fn mdns_browse(wanted: &Subject, tx: &mpsc::Sender<SocketAddrV4>, deadline: std::time::Instant) {
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
        if !advertises(&service, wanted) {
            continue;
        }
        for ip in service.get_addresses_v4() {
            let addr = SocketAddrV4::new(ip, service.get_port());
            // mDNS queries go to multicast, never to loopback.
            if plausible_sender(addr, false) && seen.insert(addr) && tx.blocking_send(addr).is_err()
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
