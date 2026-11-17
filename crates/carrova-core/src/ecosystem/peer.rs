//! The supervisor's view of each known device: who it is, its policy here,
//! where presence last saw it and its live link, if any.

use std::collections::VecDeque;
use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::{Duration, Instant};

use carrova_crypto::identity::{DeviceId, DevicePublic};
use carrova_crypto::pairing::KnownSecret;
use carrova_link::{Feature, FeatureState, FileOffer, PolicyDoc, SendOutcome};
use carrova_platform::{OsKind, Platform};
use carrova_transport::{Connection, KnownPeers, browse};

use super::phone::mirror::Mirror;
use super::supervisor::Shared;
use crate::cancel::Cancel;
use crate::devices::DeviceRecord;
use crate::lan::{BROWSE_WINDOW, known_by_recent_use};

/// Where linkable devices are on the network right now.
pub trait PeerSource: Send + Sync {
    /// Known devices present now, each with its link listener's address.
    /// Called once per supervisor round, from the supervisor's thread.
    fn seen(&self) -> Vec<SeenPeer>;
}

/// A known device present on the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeenPeer {
    /// Which known device.
    pub id: DeviceId,
    /// Its link listener.
    pub addr: SocketAddrV4,
}

/// The [`PeerSource`] of the real network: a presence browse, devices
/// recognised by their known-device tags (never by name), at the link
/// port their advert carries. Each call browses for [`BROWSE_WINDOW`].
#[derive(Debug)]
pub struct PresenceSource {
    platform: Arc<dyn Platform>,
}

impl PresenceSource {
    /// Browse with `platform`.
    #[must_use]
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self { platform }
    }
}

impl PeerSource for PresenceSource {
    fn seen(&self) -> Vec<SeenPeer> {
        let platform = &*self.platform;
        let found = browse(platform, BROWSE_WINDOW)
            .inspect_err(|e| tracing::debug!(error = %e, "presence browse failed"))
            .unwrap_or_default();
        let known = known_by_recent_use(platform, usize::MAX)
            .inspect_err(|e| tracing::warn!(error = %e, "known devices unreadable"))
            .unwrap_or_default();
        found
            .into_iter()
            .filter_map(|n| {
                let port = n.advert.link_port?;
                let tags = n.advert.known.as_ref()?;
                let (record, _) = known.iter().find(|(r, s)| tags.made_with(s, &r.public))?;
                Some(SeenPeer {
                    id: record.public,
                    addr: SocketAddrV4::new(n.addr, port),
                })
            })
            .collect()
    }
}

/// How a known device's link stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PeerState {
    /// Connected.
    Linked,
    /// This side is trying to connect.
    Connecting,
    /// Not connected, and not being dialled now.
    Away,
}

/// A known device as the supervisor sees it ([`Supervisor::status`]).
///
/// [`Supervisor::status`]: super::Supervisor::status
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LinkedPeer {
    /// Its identity.
    pub id: DeviceId,
    /// Its local name.
    pub name: String,
    /// Its OS.
    pub os: OsKind,
    /// Its link.
    pub state: PeerState,
    /// What this side allows it.
    pub local: PolicyDoc,
    /// What it allows this side, once its policy arrived on the link.
    pub remote: Option<PolicyDoc>,
    /// The phone features it lacks the access for (it said so on the link).
    #[serde(default)]
    pub missing: Vec<Feature>,
}

/// A live link.
#[derive(Debug)]
pub(super) struct Link {
    pub(super) conn: Arc<Connection>,
    /// Which link of this peer it is (a newer one replaces it).
    pub(super) generation: u64,
    pub(super) remote: Option<PolicyDoc>,
    /// Fires when the link closes: asks on it end.
    pub(super) closed: Cancel,
    /// Accepted handoff files whose bytes are due on a `File` lane (names
    /// already safe).
    pub(super) handoff_files: Vec<FileOffer>,
    /// What the device's phone gives this side (memory only).
    pub(super) mirror: Mirror,
}

/// How many texts [`SentTexts`] remembers.
const SENT_KEPT: usize = 256;

/// Texts sent for a device by their `client_id`, newest last: `None` while
/// sending. Kept across its links, so a repeat after a lost reply is
/// answered from here, never sent twice.
#[derive(Debug, Default)]
pub(super) struct SentTexts(VecDeque<(u64, Option<SendOutcome>)>);

impl SentTexts {
    /// Start sending `client_id`: `None` if it is new, else its outcome so
    /// far (`Unknown` while still sending, as it may yet go out).
    pub(super) fn begin(&mut self, client_id: u64) -> Option<SendOutcome> {
        if let Some((_, outcome)) = self.0.iter().find(|(id, _)| *id == client_id) {
            return Some(outcome.unwrap_or(SendOutcome::Unknown));
        }
        if self.0.len() == SENT_KEPT {
            self.0.pop_front();
        }
        self.0.push_back((client_id, None));
        None
    }

    /// Record how sending `client_id` went.
    pub(super) fn finish(&mut self, client_id: u64, outcome: SendOutcome) {
        if let Some(entry) = self.0.iter_mut().find(|(id, _)| *id == client_id) {
            entry.1 = Some(outcome);
        }
    }
}

/// One known device.
#[derive(Debug)]
pub(super) struct Peer {
    pub(super) record: DeviceRecord,
    pub(super) secret: KnownSecret,
    pub(super) local: PolicyDoc,
    pub(super) link: Option<Link>,
    /// Where and when presence last saw it.
    pub(super) seen: Option<(SocketAddrV4, Instant)>,
    pub(super) dialing: bool,
    /// Wait before the next dial after a failure.
    pub(super) backoff: Duration,
    pub(super) next_dial: Instant,
    pub(super) sms_sent: SentTexts,
}

impl Peer {
    pub(super) fn new(record: DeviceRecord, secret: KnownSecret, local: PolicyDoc) -> Self {
        Self {
            record,
            secret,
            local,
            link: None,
            seen: None,
            dialing: false,
            backoff: Duration::ZERO,
            next_dial: Instant::now(),
            sms_sent: SentTexts::default(),
        }
    }

    pub(super) fn id(&self) -> DeviceId {
        self.record.public
    }

    /// Whether this side allows it any feature (only then is it dialled).
    pub(super) fn wanted(&self) -> bool {
        Feature::PHASE2
            .iter()
            .any(|&f| self.local.get(f) != FeatureState::Off)
    }

    /// Whether presence saw it within `away_after`.
    pub(super) fn present(&self, away_after: Duration) -> bool {
        self.seen.is_some_and(|(_, at)| at.elapsed() < away_after)
    }

    pub(super) fn status(&self) -> LinkedPeer {
        let state = match (&self.link, self.dialing) {
            (Some(_), _) => PeerState::Linked,
            (None, true) => PeerState::Connecting,
            (None, false) => PeerState::Away,
        };
        LinkedPeer {
            id: self.id(),
            name: self.record.name.clone(),
            os: self.record.os,
            state,
            local: self.local.clone(),
            remote: self.link.as_ref().and_then(|l| l.remote.clone()),
            missing: self
                .link
                .as_ref()
                .map(|l| l.mirror.missing())
                .unwrap_or_default(),
        }
    }
}

/// Whether this device dials `peer` (ADR 0002 §4: the lower key dials).
pub(super) fn dials(me: &DevicePublic, peer: &DevicePublic) -> bool {
    me.as_bytes() < peer.as_bytes()
}

impl KnownPeers for Shared {
    fn candidates(&self) -> Vec<(DevicePublic, KnownSecret)> {
        self.peers()
            .iter()
            .map(|p| (p.id(), p.secret.clone()))
            .collect()
    }
}

#[cfg(test)]
#[path = "../../tests/ecosystem/peer/unit.rs"]
mod tests;
