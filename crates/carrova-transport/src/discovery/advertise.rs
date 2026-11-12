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
use crate::tuning::{
    BROADCAST_MAX_DATAGRAM, MDNS_QUEUE_RETRY, MDNS_RETIRE_TRIES, MDNS_WITHDRAW_WAIT,
};
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
    /// A fresh daemon per advert: shutting the old one down also drops the
    /// answers it still had queued (with the old offer and its name).
    mdns: Option<ServiceDaemon>,
    /// Whether "mDNS unavailable" was logged: once per advertiser, not per advert.
    mdns_failure_logged: bool,
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
        Ok(Self {
            port,
            instance,
            name,
            current,
            mdns: None,
            mdns_failure_logged: false,
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

    /// Make `advert` current and register it over mDNS, on a fresh daemon,
    /// with TXT [`TXT_VERSION`] plus `txt`.
    fn publish(&mut self, advert: Advert, txt: &[(&str, String)]) {
        *self.current.lock().unwrap_or_else(PoisonError::into_inner) = Some(advert);
        // shortcut: an old daemon past MDNS_WITHDRAW_WAIT may still run, so the new one
        // renames the instance; use a fresh instance per daemon if that warning shows up.
        self.retire_mdns();
        let daemon = match ServiceDaemon::new() {
            Ok(daemon) => daemon,
            Err(e) => {
                // Later adverts retry; only the first failure is loud.
                if std::mem::replace(&mut self.mdns_failure_logged, true) {
                    tracing::debug!(error = %e, "mDNS discovery still unavailable");
                } else {
                    tracing::warn!(error = %e, "mDNS discovery unavailable");
                }
                return;
            }
        };
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
        if let Err(e) = info.and_then(|info| daemon.register(info)) {
            tracing::warn!(error = %e, "mDNS advertisement failed");
        }
        self.mdns = Some(daemon);
    }

    /// Withdraw the current daemon. One whose shutdown the command queue
    /// refused would keep advertising if dropped, so it is retried off-thread.
    fn retire_mdns(&mut self) {
        if let Some(daemon) = self.mdns.take()
            && !withdraw(&daemon)
        {
            let retry = std::thread::Builder::new()
                .name("mdns-retire".into())
                .spawn(move || {
                    // `retire_mdns` already made the first try.
                    if !(1..MDNS_RETIRE_TRIES).any(|_| withdraw(&daemon)) {
                        tracing::warn!("mDNS daemon still advertising: shutdown refused");
                    }
                });
            if let Err(e) = retry {
                tracing::warn!(error = %e, "mDNS daemon still advertising: no retry thread");
            }
        }
    }
}

impl std::fmt::Debug for Advertiser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Advertiser")
            .field("port", &self.port)
            .field("instance", &self.instance)
            .field("mdns", &self.mdns.is_some())
            .finish_non_exhaustive()
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        if let Some(task) = self.responder.take() {
            task.abort();
        }
        self.retire_mdns();
    }
}

/// Shut `daemon` down and wait until it sent its goodbyes and dropped the
/// answers it had queued: no answer from it goes out afterwards. Returns
/// whether nothing is left to retry: the shutdown was queued (an ack past the
/// wait counts) or the daemon is gone; `false` means the queue refused it.
fn withdraw(daemon: &ServiceDaemon) -> bool {
    let shutdown = daemon.shutdown().or_else(|e| match e {
        mdns_sd::Error::Again => {
            std::thread::sleep(MDNS_QUEUE_RETRY);
            daemon.shutdown()
        }
        e => Err(e),
    });
    let done = shutdown
        .as_ref()
        .map(|done| done.recv_timeout(MDNS_WITHDRAW_WAIT).is_ok());
    // A refused shutdown is reported once, by whoever gives up retrying it.
    if !matches!(done, Ok(true)) && !matches!(shutdown, Err(mdns_sd::Error::Again)) {
        tracing::warn!("mDNS daemon did not withdraw its advertisement");
    }
    !matches!(shutdown, Err(mdns_sd::Error::Again))
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
