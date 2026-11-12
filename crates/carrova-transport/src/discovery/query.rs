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
use super::presence::PresenceAdvert;
use super::seek::{Seek, SeekKey, Subject};
use crate::error::Result;
use crate::runtime::runtime;
use crate::tuning::{BROADCAST_MAX_DATAGRAM, BROADCAST_RETRY, MDNS_POLL};
use crate::{BROADCAST_PORT, BROADCAST_REPLY, PROTOCOL_VERSION, SERVICE_TYPE, TXT_VERSION};

/// What a peer said about itself in a discovery answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(super) struct Announced {
    /// Its advert (empty if it gave nothing).
    pub(super) advert: PresenceAdvert,
    /// Whether it answered as a sender offering a pairing code (not as a
    /// presence).
    pub(super) offers_code: bool,
}

/// What a discovery answer yields, from a plausible sender `addr` that
/// announced itself.
type Make<T> = fn(SocketAddrV4, Announced) -> T;

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
            let (SocketAddr::V4(from), Some((port, announced))) = (from, parse_reply(&buf[..n]))
            else {
                continue;
            };
            let candidate = SocketAddrV4::new(*from.ip(), port);
            if !plausible_sender(candidate, loopback_queried) {
                tracing::debug!(%candidate, "implausible broadcast answer ignored");
                continue;
            }
            if tx.send(make(candidate, announced)).await.is_err() {
                return;
            }
        }
    }
}

/// A broadcast reply's port and what it announced: a sender's offer gives
/// at most a name after a space (answering a browse); a presence gives TXT
/// entries, `'\n'` before each (the first of a repeated key counts).
fn parse_reply(datagram: &[u8]) -> Option<(u16, Announced)> {
    let body = std::str::from_utf8(datagram.strip_prefix(BROADCAST_REPLY)?).ok()?;
    let (port, announced) = if let Some((port, entries)) = body.split_once('\n') {
        let entries: Vec<_> = entries
            .split('\n')
            .filter_map(|e| e.split_once('='))
            .collect();
        let get = |key: &str| entries.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
        let advert = PresenceAdvert::from_txt(get);
        (
            port,
            Announced {
                advert,
                offers_code: false,
            },
        )
    } else {
        let (port, name) = body
            .split_once(' ')
            .map_or((body, None), |(port, name)| (port, Some(name.to_owned())));
        let advert = PresenceAdvert {
            name,
            ..PresenceAdvert::default()
        };
        (
            port,
            Announced {
                advert,
                offers_code: true,
            },
        )
    };
    Some((port.parse().ok()?, announced))
}

/// Does a TXT record (`get(key)`) carry a well-formed nameplate, i.e. a
/// pairing code on offer?
pub(super) fn offers_a_code<'a>(get: impl Fn(&str) -> Option<&'a str>) -> bool {
    let key = SeekKey::Nameplate;
    get(key.as_str())
        .and_then(|value| Seek::parse(key, value))
        .is_some()
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
/// address of every service `matches` accepts, with what it announced.
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
        let get = |key: &str| service.get_property_val_str(key);
        let announced = Announced {
            advert: PresenceAdvert::from_txt(get),
            offers_code: offers_a_code(get),
        };
        for ip in service.get_addresses_v4() {
            let addr = SocketAddrV4::new(ip, service.get_port());
            // mDNS queries go to multicast, never to loopback.
            if plausible_sender(addr, false)
                && seen.insert(addr)
                && tx.blocking_send(make(addr, announced.clone())).is_err()
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
