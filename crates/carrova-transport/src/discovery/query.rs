//! Receiver side: broadcast queries and mDNS browsing for a sender.

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;

use carrova_platform::Platform;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::broadcast_socket;
use super::interfaces::{broadcast_targets, plausible_sender};
use super::seek::{Seek, Subject};
use crate::error::Result;
use crate::runtime::runtime;
use crate::tuning::{BROADCAST_MAX_DATAGRAM, BROADCAST_RETRY, MDNS_POLL};
use crate::{
    BROADCAST_PORT, BROADCAST_REPLY, PROTOCOL_VERSION, SERVICE_TYPE, TXT_DEVICE_NAME, TXT_VERSION,
};

/// What a discovery answer yields, from a plausible sender `addr` that
/// announced `name` (or none).
type Make<T> = fn(SocketAddrV4, Option<String>) -> T;

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
pub(super) async fn broadcast_query<T>(
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
pub(super) fn mdns_browse<T>(
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
#[path = "../../tests/discovery/query/unit.rs"]
mod tests;
