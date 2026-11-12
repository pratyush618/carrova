//! Browsing: which devices nearby are offering a code or announcing their
//! presence ([`browse`]).

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use carrova_platform::Platform;
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::presence::PresenceAdvert;
use super::query::{Announced, broadcast_query, mdns_browse, offers_a_code};
use crate::error::Result;
use crate::runtime::runtime;
use crate::{BROADCAST_BROWSE, PROTOCOL_VERSION, TXT_VERSION};

/// A device nearby: a sender offering a pairing code (its name only), or a
/// presence ([`PresenceAdvert`]). Everything in it is peer-provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nearby {
    /// What it announced about itself.
    pub advert: PresenceAdvert,
    /// Whether it is a sender offering a pairing code (else a presence).
    pub offers_code: bool,
    /// How long after the browse started it first answered.
    pub answered: Duration,
    /// The address it first answered from.
    pub addr: Ipv4Addr,
}

/// Senders offering a code and presences on the LAN, collected for
/// `timeout` over mDNS and broadcast; each device once although it may
/// answer on several addresses (an offer by port, a tagged presence by its
/// advert, an untagged one also by address). Adverts with neither a
/// name nor known-device tags are left out. Never reveals codes (adverts
/// carry the nameplate and device name only).
///
/// # Errors
///
/// Discovery could not start at all (no runtime).
pub fn browse(platform: &dyn Platform, timeout: Duration) -> Result<Vec<Nearby>> {
    let rt = runtime()?;
    let start = Instant::now();
    let deadline = start + timeout;
    let (tx, mut rx) = mpsc::channel::<(SocketAddrV4, Announced)>(16);
    let mdns_tx = tx.clone();
    rt.spawn_blocking(move || {
        mdns_browse(browsable, &mdns_tx, deadline.into_std(), |a, n| (a, n));
    });
    Ok(rt.block_on(async {
        let query = async move {
            broadcast_query(platform, BROADCAST_BROWSE, &tx, deadline, |a, n| (a, n)).await;
        };
        let collect = async {
            let mut seen = HashSet::new();
            let mut nearby = Vec::new();
            while let Some((addr, announced)) = rx.recv().await {
                let advert = &announced.advert;
                let telling = advert.name.is_some() || advert.known.is_some();
                if telling && seen.insert(same_device(addr, &announced)) {
                    nearby.push(Nearby {
                        advert: announced.advert,
                        offers_code: announced.offers_code,
                        answered: start.elapsed(),
                        addr: *addr.ip(),
                    });
                }
            }
            nearby
        };
        tokio::join!(query, collect).1
    }))
}

/// Which answers come from one device: a sender's offer by port (it may
/// answer on several addresses); a presence with tags by its advert (a
/// fresh nonce per advert); a presence without tags also by address (two
/// devices may give the same name and OS).
fn same_device(addr: SocketAddrV4, announced: &Announced) -> (Option<Ipv4Addr>, u16, Announced) {
    let untagged_presence = !announced.offers_code && announced.advert.known.is_none();
    let ip = untagged_presence.then_some(*addr.ip());
    (ip, addr.port(), announced.clone())
}

/// Does a resolved mDNS service (current protocol version) advertise a
/// pairing code (a well-formed nameplate) or a presence (known-device tags
/// or an OS)?
fn browsable(service: &mdns_sd::ResolvedService) -> bool {
    let get = |key: &str| service.get_property_val_str(key);
    let presence = PresenceAdvert::from_txt(get);
    get(TXT_VERSION) == Some(PROTOCOL_VERSION.to_string().as_str())
        && (offers_a_code(get) || presence.known.is_some() || presence.os.is_some())
}

#[cfg(test)]
#[path = "../../tests/discovery/browse/unit.rs"]
mod tests;
