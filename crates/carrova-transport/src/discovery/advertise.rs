//! The mDNS registration and broadcast responder for a sender's current
//! offer, or for a device's presence.

use std::sync::{Arc, Mutex, PoisonError};

use carrova_platform::Platform;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::task::JoinHandle;

use super::broadcast_socket;
use super::presence::PresenceAdvert;
use super::seek::Offer;
use crate::error::Result;
use crate::runtime::runtime;
use crate::tuning::{BROADCAST_MAX_DATAGRAM, MDNS_WITHDRAW_WAIT};
use crate::{
    BROADCAST_BROWSE, BROADCAST_PORT, BROADCAST_REPLY, PROTOCOL_VERSION, SERVICE_TYPE,
    TXT_DEVICE_NAME, TXT_VERSION,
};

/// What an [`Advertiser`] announces.
#[derive(Debug, Clone)]
pub(crate) enum Advert {
    /// A sender's offer (pairing code or resumable session).
    Offer(Offer),
    /// A device's presence.
    Presence(PresenceAdvert),
}

/// An advertisement: mDNS registration plus broadcast responder. Failing
/// parts are logged and skipped (a fallback ticket still works).
/// Everything stops on drop.
pub(crate) struct Advertiser {
    port: u16,
    /// mDNS instance and host label (unique per listener).
    instance: String,
    /// Device name announced while a code is offered ([`txt_fit`]).
    name: Option<String>,
    current: Arc<Mutex<Option<Advert>>>,
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
        let mut txt = vec![(offer.advert().key().as_str(), offer.advert().value())];
        let name = self.name.clone().filter(|_| offer.is_pairing());
        txt.extend(name.map(|n| (TXT_DEVICE_NAME, n)));
        self.publish(Advert::Offer(offer), &txt);
    }

    /// Replace the advertisement with `presence`.
    pub(crate) fn set_presence(&mut self, presence: PresenceAdvert) {
        let txt = presence.txt();
        self.publish(Advert::Presence(presence), &txt);
    }

    /// Make `advert` current and register it over mDNS with TXT
    /// [`TXT_VERSION`] plus `txt`.
    fn publish(&mut self, advert: Advert, txt: &[(&str, String)]) {
        *self.current.lock().unwrap_or_else(PoisonError::into_inner) = Some(advert);
        let Some(daemon) = &self.mdns else { return };
        if let Some(old) = self.registered.take() {
            // Wait until the daemon thread withdrew it: until then it still
            // answers queries with the old offer (and its name).
            let withdrawn = daemon
                .unregister(&old)
                .map(|done| done.recv_timeout(MDNS_WITHDRAW_WAIT).is_ok());
            if !matches!(withdrawn, Ok(true)) {
                tracing::warn!("mDNS daemon did not withdraw the previous advertisement");
            }
        }
        let version = PROTOCOL_VERSION.to_string();
        let txt: Vec<(&str, &str)> = std::iter::once((TXT_VERSION, version.as_str()))
            .chain(txt.iter().map(|(k, v)| (*k, v.as_str())))
            .collect();
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
    cut(name, usize::from(u8::MAX) - TXT_DEVICE_NAME.len() - 1)
}

/// The longest prefix of `s` of at most `max` bytes, on a char boundary.
pub(super) fn cut(s: &str, max: usize) -> &str {
    let mut end = s.len().min(max);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// The reply to broadcast `query` while `advert` is current: the port for
/// a query naming the offer; port and `name` for a browse query while a
/// code is offered; port and TXT entries (`'\n'` before each) for a browse
/// query to a presence; else none.
fn reply(advert: &Advert, query: &[u8], port: u16, name: Option<&str>) -> Option<Vec<u8>> {
    let mut reply = BROADCAST_REPLY.to_vec();
    reply.extend_from_slice(port.to_string().as_bytes());
    let offer = match advert {
        Advert::Offer(offer) => offer,
        Advert::Presence(_) if query != BROADCAST_BROWSE => return None,
        Advert::Presence(presence) => {
            for (key, value) in presence.txt() {
                reply.extend_from_slice(format!("\n{key}={value}").as_bytes());
            }
            return Some(reply);
        }
    };
    if offer.answers(query) {
        return Some(reply);
    }
    let name = name.filter(|_| query == BROADCAST_BROWSE && offer.is_pairing())?;
    reply.push(b' ');
    reply.extend_from_slice(name.as_bytes());
    Some(reply)
}

/// Answer broadcast queries that ask for the current offer, and browse
/// queries while a code is offered or to a presence.
async fn respond(
    socket: tokio::net::UdpSocket,
    current: Arc<Mutex<Option<Advert>>>,
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
            .and_then(|advert| reply(advert, &buf[..n], port, name.as_deref()));
        if let Some(answer) = answer
            && let Err(e) = socket.send_to(&answer, from).await
        {
            tracing::debug!(error = %e, "broadcast reply failed");
        }
    }
}

#[cfg(test)]
#[path = "../../tests/discovery/advertise/unit.rs"]
mod tests;
