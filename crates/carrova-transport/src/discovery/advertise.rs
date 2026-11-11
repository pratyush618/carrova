//! Sender side: the mDNS registration and broadcast responder for the
//! current offer.

use std::sync::{Arc, Mutex, PoisonError};

use carrova_platform::Platform;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::task::JoinHandle;

use super::broadcast_socket;
use super::seek::Offer;
use crate::error::Result;
use crate::runtime::runtime;
use crate::tuning::BROADCAST_MAX_DATAGRAM;
use crate::{
    BROADCAST_BROWSE, BROADCAST_PORT, BROADCAST_REPLY, PROTOCOL_VERSION, SERVICE_TYPE,
    TXT_DEVICE_NAME, TXT_VERSION,
};

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

#[cfg(test)]
#[path = "../../tests/discovery/advertise/unit.rs"]
mod tests;
