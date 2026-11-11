//! Receiver-side browsing: which senders nearby are offering a code
//! ([`browse`]).

use std::collections::HashSet;
use std::net::SocketAddrV4;
use std::time::Duration;

use carrova_platform::Platform;
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::query::{broadcast_query, mdns_browse};
use super::seek::{Seek, SeekKey};
use crate::error::Result;
use crate::runtime::runtime;
use crate::{BROADCAST_BROWSE, PROTOCOL_VERSION, TXT_VERSION};

/// A sender nearby that is offering a pairing code. Its device name is
/// advertised only while a code is offered (peer-provided: sanitise
/// before display).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nearby {
    /// The sender's device name as it advertised it.
    pub device_name: String,
}

/// Senders offering a code on the LAN, collected for `timeout` over mDNS
/// and broadcast; each sender once (by QUIC port and name, as it may answer
/// on several addresses). Senders that announce no name are left out.
/// Never reveals codes (adverts carry the nameplate and device name only).
///
/// # Errors
///
/// Discovery could not start at all (no runtime).
pub fn browse(platform: &dyn Platform, timeout: Duration) -> Result<Vec<Nearby>> {
    let rt = runtime()?;
    let deadline = Instant::now() + timeout;
    let (tx, mut rx) = mpsc::channel::<(SocketAddrV4, Option<String>)>(16);
    let mdns_tx = tx.clone();
    rt.spawn_blocking(move || {
        mdns_browse(offers_a_code, &mdns_tx, deadline.into_std(), |a, n| (a, n));
    });
    Ok(rt.block_on(async {
        let query = async move {
            broadcast_query(platform, BROADCAST_BROWSE, &tx, deadline, |a, n| (a, n)).await;
        };
        let collect = async {
            let mut seen = HashSet::new();
            let mut nearby = Vec::new();
            while let Some((addr, name)) = rx.recv().await {
                if let Some(name) = name
                    && seen.insert((addr.port(), name.clone()))
                {
                    nearby.push(Nearby { device_name: name });
                }
            }
            nearby
        };
        tokio::join!(query, collect).1
    }))
}

/// Does a resolved mDNS service advertise a pairing code (current protocol
/// version, a well-formed nameplate)?
fn offers_a_code(service: &mdns_sd::ResolvedService) -> bool {
    let key = SeekKey::Nameplate;
    service.get_property_val_str(TXT_VERSION) == Some(PROTOCOL_VERSION.to_string().as_str())
        && service
            .get_property_val_str(key.as_str())
            .and_then(|value| Seek::parse(key, value))
            .is_some()
}
