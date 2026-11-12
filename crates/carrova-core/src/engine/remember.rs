//! Remembering the peer of a successful code-paired move (ADR 0001 §4):
//! both sides store each other, latest pairing wins.

use std::net::IpAddr;

use carrova_crypto::identity::Fingerprint;
use carrova_transport::{Connection, PeerIdentity};

use super::session::Session;
use crate::devices::{DeviceRecord, HintUpdate, KnownDevices};
use crate::error::Result;
use crate::lan::clean_device_name;
use crate::session::Event;
use crate::store::Source;
use crate::time;

/// A code-paired peer of a move: its identity and where it was reached.
#[derive(Debug)]
pub(super) struct KnownPeer {
    identity: PeerIdentity,
    addr: IpAddr,
}

impl KnownPeer {
    /// The peer of `conn`, if it proved a device identity.
    pub(super) fn of(conn: &Connection) -> Option<Self> {
        Some(Self {
            identity: conn.peer_identity()?.clone(),
            addr: conn.peer_addr().ip(),
        })
    }
}

impl Session {
    /// After a successful move with a code-paired `peer`: remember it,
    /// count the move and tell the front-end
    /// ([`Event::DeviceRemembered`]). The move already succeeded, so a
    /// failure here is only a warning.
    pub(super) fn remember_peer(&self, peer: &KnownPeer) {
        if let Err(e) = self.try_remember_peer(peer) {
            tracing::warn!(error = %e, "could not remember the device moved with");
            self.emit(Event::Warning {
                message: format!("could not remember the other computer: {e}"),
            });
        }
    }

    fn try_remember_peer(&self, peer: &KnownPeer) -> Result<()> {
        let platform = self.platform();
        let KnownPeer { identity, addr } = peer;
        let fingerprint = Fingerprint::of(&identity.public);
        // Both sides learn the peer's OS before any move can finish.
        let Some(os) = self.record.peer_os else {
            tracing::warn!("peer's OS unknown; not remembering it");
            return Ok(());
        };
        let announced = match &self.record.source {
            Some(Source::Lan { peer_name, .. }) => peer_name.as_deref(),
            _ => None,
        };
        let name = announced
            .and_then(clean_device_name)
            .unwrap_or_else(|| fingerprint.to_string());
        let now = time::now_unix();
        let known = KnownDevices::open(platform)?;
        let record = DeviceRecord {
            public: identity.public,
            name,
            os,
            first_seen: now,
            last_move: None,
            moves: 0,
            mac_hint: None,
        };
        // Off never collects and drops what was saved (ADR 0001).
        let hint = if self.engine.wol_hints {
            platform
                .neighbour_mac(*addr)
                .map_or(HintUpdate::Keep, HintUpdate::Set)
        } else {
            HintUpdate::Clear
        };
        let record = known.remember_move(platform, record, &identity.secret, hint, now)?;
        tracing::debug!(device = %fingerprint, "remembered the device moved with");
        self.emit(Event::DeviceRemembered {
            name: record.name,
            fingerprint: fingerprint.to_string(),
        });
        Ok(())
    }
}
