//! Keeping links up (spec "The link"): the listener every known device may
//! link to, and the rounds that follow the known devices and presence and
//! dial. The lower key dials (ADR 0002 §4), only while some feature is on
//! for the device here and presence saw it within
//! [`SupervisorConfig::away_after`]; failures back off from
//! [`SupervisorConfig::backoff_min`], doubling to
//! [`SupervisorConfig::backoff_max`], reset by a network change.
//!
//! While any device is wanted, the process asks to keep running (a phone's
//! link service, "Linked to" its names); with none, it lets the OS stop it.
//! While the user has paused linking ([`Platform::links_paused`]) every
//! link is closed and none is dialled or accepted, until they resume.
//!
//! [`SupervisorConfig::away_after`]: super::SupervisorConfig::away_after
//! [`SupervisorConfig::backoff_min`]: super::SupervisorConfig::backoff_min
//! [`SupervisorConfig::backoff_max`]: super::SupervisorConfig::backoff_max
//! [`Platform::links_paused`]: carrova_platform::Platform::links_paused

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use carrova_crypto::identity::DeviceId;
use carrova_crypto::pairing::KnownSecret;
use carrova_link::{LinkError, PolicyDoc};
use carrova_platform::LinkNotice;
use carrova_transport::{
    ConnectOptions, Connection, KnownPeers, ListenOptions, Listener, Target, TransportError,
    connect,
};

use super::PolicyStore;
use super::clip::lock_clips;
use super::features;
use super::peer::{Link, Peer, PeerSource, dials};
use super::phone::mirror::{self, Mirror};
use super::supervisor::Shared;
use crate::cancel::Cancel;
use crate::error::Result;
use crate::lan::known_by_recent_use;
use crate::session::Event;

/// How long one dial may take.
const DIAL_TIMEOUT: Duration = Duration::from_secs(5);
/// How long one accept waits before waiting again (a stop ends it at once).
const ACCEPT_WAIT: Duration = Duration::from_secs(600);

/// Links made so far (each link's generation).
static LINKS: AtomicU64 = AtomicU64::new(0);

/// The link listener: no advert of its own (presence carries its port).
pub(super) fn bind(shared: &Shared) -> Result<Listener> {
    Ok(Listener::bind(ListenOptions {
        port: 0,
        discovery: false,
        device_name: None,
        platform: Arc::clone(shared.platform()),
        device: Some(Arc::clone(&shared.key)),
        cancel: Some(shared.stop.signal()),
    })?)
}

/// Accept links from known devices until the supervisor stops.
pub(super) fn accept_loop(shared: &Arc<Shared>, mut listener: Listener) {
    let known: Arc<dyn KnownPeers> = Arc::<Shared>::clone(shared);
    loop {
        match listener.accept_link(Arc::clone(&known), ACCEPT_WAIT) {
            Ok(conn) => register(shared, conn),
            Err(TransportError::Cancelled) => return,
            Err(TransportError::Timeout) => {}
            Err(e) => {
                tracing::debug!(error = %e, "link listener failed");
                if shared.stop.sleep(shared.config.tick).is_err() {
                    return;
                }
            }
        }
    }
}

/// Run rounds every [`SupervisorConfig::tick`](super::SupervisorConfig::tick)
/// until the supervisor stops.
pub(super) fn maintain(shared: &Arc<Shared>, source: &dyn PeerSource) {
    let mut lan = interfaces(shared);
    // `None` until the first round, so the first notice is always given.
    let mut shown: Option<Option<LinkNotice>> = None;
    loop {
        if let Err(e) = refresh(shared) {
            tracing::warn!(error = %e, "known devices unreadable");
        }
        let paused = shared.platform().links_paused();
        let notice = notice(shared, paused);
        if shown.as_ref() != Some(&notice) {
            // Recorded only once given, so a refusal is asked again next round.
            match shared.platform().keep_running(notice.as_ref()) {
                Ok(()) => shown = Some(notice),
                Err(e) => tracing::warn!(error = %e, "could not keep running for the links"),
            }
        }
        let seen = source.seen();
        let now = interfaces(shared);
        if now != lan {
            lan = now;
            for peer in shared.peers().iter_mut() {
                peer.backoff = Duration::ZERO;
                peer.next_dial = Instant::now();
            }
        }
        round(shared, &seen, paused);
        if shared.stop.sleep(shared.config.tick).is_err() {
            return;
        }
    }
}

/// What to keep running for: the devices some feature is on for here, and
/// whether linking is paused; `None` with no such device.
fn notice(shared: &Shared, paused: bool) -> Option<LinkNotice> {
    let devices: Vec<String> = shared
        .peers()
        .iter()
        .filter(|peer| peer.wanted())
        .map(|peer| peer.record.name.clone())
        .collect();
    (!devices.is_empty()).then_some(LinkNotice { devices, paused })
}

/// This device's LAN addresses (a change resets every backoff).
fn interfaces(shared: &Shared) -> Vec<Ipv4Addr> {
    let found = shared.platform().lan_ipv4_interfaces();
    found.map_or_else(|_| Vec::new(), |i| i.into_iter().map(|i| i.ip).collect())
}

/// Follow the known devices: add new ones (with their stored policy),
/// update the others, and drop forgotten ones with their links (and the
/// clipboard history).
pub(super) fn refresh(shared: &Shared) -> Result<()> {
    let platform = &**shared.platform();
    let known = known_by_recent_use(platform, usize::MAX)?;
    let present: Vec<DeviceId> = shared.peers().iter().map(Peer::id).collect();
    let policies = PolicyStore::open(platform)?;
    let mut new: HashMap<DeviceId, PolicyDoc> = HashMap::new();
    for (record, _) in known.iter().filter(|(r, _)| !present.contains(&r.public)) {
        let policy = policies.get(platform, &record.public).unwrap_or_else(|e| {
            // One bad file must not stop the others: that device gets nothing.
            tracing::warn!(device = ?record.public, error = %e, "policy unreadable: all off");
            PolicyDoc::computer_defaults()
        });
        new.insert(record.public, policy);
    }
    let forgotten = {
        let mut peers = shared.peers();
        let mut old = std::mem::take(&mut *peers);
        for (record, secret) in known {
            let peer = match old.iter().position(|p| p.id() == record.public) {
                Some(i) => {
                    let mut peer = old.swap_remove(i);
                    peer.record = record;
                    peer.secret = secret;
                    peer
                }
                None => {
                    let local = new
                        .remove(&record.public)
                        .unwrap_or_else(PolicyDoc::computer_defaults);
                    Peer::new(record, secret, local)
                }
            };
            peers.push(peer);
        }
        old
    };
    if forgotten.is_empty() {
        return Ok(());
    }
    lock_clips(shared).clear();
    for mut peer in forgotten {
        if let Some(link) = peer.link.take() {
            close(&link);
            shared.emit(Event::LinkDown {
                device: peer.id(),
                retrying: false,
            });
            mirror::cleared(shared, peer.id(), link.mirror.held());
        }
    }
    Ok(())
}

/// Note where presence saw each device, close the links this side no
/// longer wants (all of them while `paused`), and dial the devices due.
fn round(shared: &Arc<Shared>, seen: &[super::SeenPeer], paused: bool) {
    let now = Instant::now();
    let away_after = shared.config.away_after;
    let mut down = Vec::new();
    let mut due = Vec::new();
    for peer in shared.peers().iter_mut() {
        if let Some(s) = seen.iter().find(|s| s.id == peer.id()) {
            peer.seen = Some((s.addr, now));
        }
        if paused {
            down.extend(peer.link.take().map(|link| (peer.id(), link)));
            continue;
        }
        if !dials(&shared.me, &peer.id()) {
            continue;
        }
        let wanted = peer.wanted() && peer.present(away_after);
        match (&peer.link, peer.seen) {
            (Some(_), _) if !wanted => {
                down.extend(peer.link.take().map(|link| (peer.id(), link)));
            }
            (None, Some((addr, _))) if wanted && !peer.dialing && now >= peer.next_dial => {
                peer.dialing = true;
                due.push((peer.id(), peer.secret.clone(), addr));
            }
            _ => {}
        }
    }
    for (device, link) in down {
        close(&link);
        shared.emit(Event::LinkDown {
            device,
            retrying: false,
        });
        mirror::cleared(shared, device, link.mirror.held());
    }
    for (device, secret, addr) in due {
        let dialer = Arc::clone(shared);
        let started = shared.spawn("carrova-link-dial", move || {
            dial(&dialer, device, &secret, addr);
        });
        if started.is_err() {
            failed(shared, &device);
        }
    }
}

/// Dial `device` at `addr` once.
fn dial(shared: &Arc<Shared>, device: DeviceId, secret: &KnownSecret, addr: SocketAddrV4) {
    let target = Target::Link {
        peer: &device,
        secret,
        addr,
    };
    let opts = ConnectOptions {
        platform: Arc::clone(shared.platform()),
        device: Some(Arc::clone(&shared.key)),
        cancel: Some(shared.stop.signal()),
    };
    match connect(target, DIAL_TIMEOUT, &opts) {
        Ok(conn) => register(shared, conn),
        Err(e) => {
            // Any failure (an older peer, a refusal, nobody there) is
            // "away": retried after the backoff, never an error loop.
            tracing::debug!(error = %e, "link dial failed");
            failed(shared, &device);
        }
    }
}

/// A dial of `device` failed: wait longer before the next.
fn failed(shared: &Shared, device: &DeviceId) {
    let config = shared.config;
    if let Some(peer) = shared.peers().iter_mut().find(|p| p.id() == *device) {
        peer.dialing = false;
        peer.backoff = if peer.backoff.is_zero() {
            config.backoff_min
        } else {
            (peer.backoff * 2).min(config.backoff_max)
        };
        peer.next_dial = Instant::now() + peer.backoff;
    }
}

/// Make `conn` the device's live link, closing an older one as
/// superseded, and serve it on its own thread. Refused while linking is
/// paused.
fn register(shared: &Arc<Shared>, conn: Connection) {
    let Some(device) = conn.peer_identity().map(|p| p.public) else {
        return;
    };
    if shared.platform().links_paused() {
        if let Some(peer) = shared.peers().iter_mut().find(|p| p.id() == device) {
            peer.dialing = false;
        }
        conn.close_now();
        return;
    }
    let conn = Arc::new(conn);
    let generation = LINKS.fetch_add(1, Ordering::Relaxed);
    let closed = Cancel::default();
    let (fresh, held) = {
        let mut peers = shared.peers();
        // Checked under the lock `close_all` takes after the stop fires.
        let peer = peers.iter_mut().find(|p| p.id() == device);
        let Some(peer) = peer.filter(|_| !shared.stop.is_cancelled()) else {
            conn.close_now();
            return;
        };
        peer.dialing = false;
        peer.backoff = Duration::ZERO;
        let older = peer.link.replace(Link {
            conn: Arc::clone(&conn),
            generation,
            remote: None,
            closed: closed.clone(),
            handoff_files: Vec::new(),
            sms_sent: HashMap::new(),
            mirror: Mirror::default(),
        });
        if let Some(older) = &older {
            older.conn.close_superseded();
            older.closed.cancel();
        }
        let held = older.as_ref().map(|l| l.mirror.held()).unwrap_or_default();
        (older.is_none(), held)
    };
    // The newer link's phone resends its state.
    mirror::cleared(shared, device, held);
    if fresh {
        shared.emit(Event::LinkUp { device });
    }
    let (link, pushing, pushed_conn) = (Arc::clone(shared), Arc::clone(shared), Arc::clone(&conn));
    let started = shared
        .spawn("carrova-link", move || {
            let ended = features::serve(&link, device, &conn, generation, &closed);
            ended_link(&link, device, generation, &ended);
        })
        .and_then(|()| {
            shared.spawn("carrova-link-policy", move || {
                features::push_policy(&pushing, &device, &pushed_conn);
            })
        });
    if let Err(e) = started {
        tracing::warn!(error = %e, "link not served");
    }
}

/// The link `generation` of `device` ended with `why`: unless something
/// here closed or replaced it already, it is down, and redialled at once
/// by the dialling side (after the first backoff when superseded: a newer
/// link from this device lives elsewhere, which this one then replaces
/// once, not in a loop).
fn ended_link(shared: &Shared, device: DeviceId, generation: u64, why: &LinkError) {
    let (retrying, held) = {
        let mut peers = shared.peers();
        let Some(peer) = peers.iter_mut().find(|p| p.id() == device) else {
            return;
        };
        // Stopping: the stop closes every link itself, after the listener.
        if shared.stop.is_cancelled()
            || peer.link.as_ref().map(|l| l.generation) != Some(generation)
        {
            return;
        }
        let held = peer.link.take().map_or_else(Vec::new, |link| {
            close(&link);
            link.mirror.held()
        });
        if matches!(why, LinkError::Transport(TransportError::Superseded)) {
            peer.backoff = shared.config.backoff_min;
            peer.next_dial = Instant::now() + peer.backoff;
        }
        (dials(&shared.me, &device) && peer.wanted(), held)
    };
    tracing::debug!(error = %why, "link ended");
    shared.emit(Event::LinkDown { device, retrying });
    mirror::cleared(shared, device, held);
}

/// Close `link` now: its lanes and asks end.
fn close(link: &Link) {
    link.closed.cancel();
    link.conn.close_now();
}

/// Close every link without reporting it (the supervisor is stopping).
pub(super) fn close_all(shared: &Shared) {
    let links: Vec<Link> = shared
        .peers()
        .iter_mut()
        .filter_map(|p| p.link.take())
        .collect();
    links.iter().for_each(close);
}
