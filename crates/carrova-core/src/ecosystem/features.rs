//! What runs over a link: the lanes a device opens to this one (each
//! checked against this side's policy before anything past its first
//! frame is read), and the policy this side sends.
//!
//! Never logs clipboard contents, file names or URLs: file-lane failures
//! are logged by [`FileFailure`] only (their errors may hold a path).

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use carrova_crypto::identity::DeviceId;
use carrova_link::{
    ClipItem, Feature, FeatureState, FileOffer, LaneGate, LaneKind, LinkError, OfferAnswer,
    ReceivedHandoff, RefuseReason, RingMsg, accept_lane, open_lane, receive_file, recv_clip,
    recv_handoff, recv_policy, recv_ring, send_policy, serve_ping,
};
use carrova_platform::{ClipContent, ClipKind, PlatformError};
use carrova_transport::{Connection, ProtocolViolation, Stream, TransportError};
use serde::{Deserialize, Serialize};

use super::asks::{Answer, AskKind};
use super::clip::lock_clips;
use super::sink::Download;
use super::supervisor::Shared;
use crate::cancel::Cancel;
use crate::error::{CoreError, Result};
use crate::session::Event;

/// How long a lane loop waits for the next lane before waiting again
/// (long: a lane arriving at the deadline would be dropped).
const LANE_WAIT: Duration = Duration::from_secs(600);
/// How long a policy document may take to arrive on its lane.
const POLICY_WAIT: Duration = Duration::from_secs(10);
/// Longest media type passed on to the OS.
const MAX_MIME: usize = 127;

/// Why a file did not arrive ([`Event::FileFailed`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum FileFailure {
    /// The link dropped or the other device stopped answering.
    DeviceLeft,
    /// The other device said no, did not answer, or does not take files.
    Refused,
    /// Reading, writing or the transfer itself failed.
    Failed,
}

impl FileFailure {
    pub(super) fn of(e: &CoreError) -> Self {
        match e {
            CoreError::Link(e) => Self::of_link(e),
            CoreError::Transport(_) | CoreError::NotLinked => Self::DeviceLeft,
            CoreError::FeatureOff(_) => Self::Refused,
            _ => Self::Failed,
        }
    }

    fn of_link(e: &LinkError) -> Self {
        match e {
            LinkError::Transport(_) => Self::DeviceLeft,
            LinkError::Declined | LinkError::Expired | LinkError::Refused(_) => Self::Refused,
            _ => Self::Failed,
        }
    }
}

/// Largest id a JavaScript number holds exactly (2^53 - 1): offer ids
/// reach the webview in events, and come back from it for a retry.
const MAX_JS_ID: u64 = (1 << 53) - 1;

/// A random offer id, exact as a JavaScript number.
pub(super) fn offer_id() -> Result<u64> {
    let mut b = [0; 8];
    getrandom::fill(&mut b).map_err(|_| CoreError::Random)?;
    Ok(u64::from_le_bytes(b) & MAX_JS_ID)
}

/// Send this side's policy for `device` on a new `Policy` lane; a failure
/// is logged (the link dropping redoes it at the next link).
pub(super) fn push_policy(shared: &Shared, device: &DeviceId, conn: &Connection) {
    let Some(doc) = shared
        .peers()
        .iter()
        .find(|p| p.id() == *device)
        .map(|p| p.local.clone())
    else {
        return;
    };
    let sent = open_lane(conn, LaneKind::Policy).and_then(|mut s| send_policy(&mut s, &doc));
    if let Err(e) = sent {
        tracing::debug!(error = %e, "policy not sent");
    }
}

/// Admits the lanes this side's policy for the device allows.
struct Gate<'a> {
    shared: &'a Shared,
    device: DeviceId,
}

impl Gate<'_> {
    fn local(&self, feature: Feature) -> FeatureState {
        local(self.shared, &self.device, feature)
    }
}

impl LaneGate for Gate<'_> {
    fn admit(&self, kind: LaneKind) -> std::result::Result<(), RefuseReason> {
        let off = |f| self.local(f) == FeatureState::Off;
        let refused = match kind {
            LaneKind::Policy | LaneKind::Ping => false,
            LaneKind::Clipboard => off(Feature::Clipboard),
            // A handed-off file arrives on a `File` lane too.
            LaneKind::File => off(Feature::Files) && off(Feature::Handoff),
            LaneKind::Handoff => off(Feature::Handoff),
            LaneKind::Ring => off(Feature::Ring),
            // Not served yet: phone lanes stay refused until their handlers land.
            LaneKind::Notify | LaneKind::Messages | LaneKind::Calls => true,
        };
        if refused {
            Err(RefuseReason::NotAllowed)
        } else {
            Ok(())
        }
    }
}

/// This side's setting of `feature` for `device` (off once forgotten).
fn local(shared: &Shared, device: &DeviceId, feature: Feature) -> FeatureState {
    shared
        .peers()
        .iter()
        .find(|p| p.id() == *device)
        .map_or(FeatureState::Off, |p| p.local.get(feature))
}

/// Serve the lanes `device` opens on `conn` until the link ends; returns
/// why it ended. Each lane runs on its own thread.
pub(super) fn serve(
    shared: &Arc<Shared>,
    device: DeviceId,
    conn: &Connection,
    generation: u64,
    closed: &Cancel,
) -> LinkError {
    let gate = Gate { shared, device };
    loop {
        match accept_lane(conn, &gate, LANE_WAIT) {
            Ok((kind, stream)) => {
                let lane = Lane {
                    shared: Arc::clone(shared),
                    device,
                    generation,
                    closed: closed.clone(),
                };
                if let Err(e) = shared.spawn("carrova-link-lane", move || lane.run(kind, stream)) {
                    tracing::warn!(error = %e, "lane dropped");
                }
            }
            Err(e) if ends_link(&e) => return e,
            Err(_) => {}
        }
    }
}

/// Whether `e` from accepting a lane means the connection is gone (else
/// only that lane failed, or none came in time).
fn ends_link(e: &LinkError) -> bool {
    matches!(
        e,
        LinkError::Transport(
            TransportError::PeerLost
                | TransportError::Superseded
                | TransportError::Cancelled
                | TransportError::PeerCancelled
                | TransportError::LinkFailed
                | TransportError::Violation(ProtocolViolation::PeerClosed)
                | TransportError::Io(_)
                | TransportError::Internal(_)
        )
    )
}

/// One accepted lane from `device`.
struct Lane {
    shared: Arc<Shared>,
    device: DeviceId,
    generation: u64,
    closed: Cancel,
}

impl Lane {
    fn run(self, kind: LaneKind, stream: Stream) {
        let served = match kind {
            LaneKind::Policy => self.policy(stream),
            LaneKind::Ping => serve_ping(stream).map_err(CoreError::from),
            LaneKind::Clipboard => self.clipboard(stream),
            LaneKind::File => return self.file(stream),
            LaneKind::Handoff => self.handoff(stream),
            LaneKind::Ring => self.ring(stream),
            // Refused by the gate before they get here.
            LaneKind::Notify | LaneKind::Messages | LaneKind::Calls => return,
        };
        if let Err(e) = served {
            tracing::debug!(lane = ?kind, error = %e, "lane failed");
        }
    }

    fn emit(&self, event: Event) {
        self.shared.emit(event);
    }

    /// Ask the user about `kind`; `None` once it expired or the link closed.
    fn ask(&self, kind: AskKind) -> OfferAnswer {
        let sink = &**self.shared.engine.sink();
        let expiry = self.shared.config.ask_expiry;
        match self
            .shared
            .asks
            .ask(sink, self.device, kind, expiry, &self.closed)
        {
            Some(Answer::Accept) => OfferAnswer::Accept { resume_from: 0 },
            Some(Answer::Decline) => OfferAnswer::Decline,
            None => OfferAnswer::Expired,
        }
    }

    /// Run `f` on this lane's link, if it is still the live one.
    fn with_link<T>(&self, f: impl FnOnce(&mut super::peer::Link) -> T) -> Option<T> {
        let mut peers = self.shared.peers();
        let link = peers
            .iter_mut()
            .find(|p| p.id() == self.device)?
            .link
            .as_mut()
            .filter(|l| l.generation == self.generation)?;
        Some(f(link))
    }

    fn policy(&self, mut stream: Stream) -> Result<()> {
        let doc = recv_policy(&mut stream, POLICY_WAIT)?;
        if self.with_link(|l| l.remote = Some(doc)).is_some() {
            self.emit(Event::PolicyChanged {
                device: self.device,
            });
        }
        Ok(())
    }

    fn clipboard(&self, stream: Stream) -> Result<()> {
        let item = recv_clip(stream)?;
        let content = match &item {
            ClipItem::Text(t) => (ClipKind::Text, t.as_bytes().to_vec()),
            ClipItem::Png(p) => (ClipKind::Png, p.clone()),
        };
        let content = ClipContent {
            kind: content.0,
            bytes: content.1,
            sensitive: false,
        };
        let note = {
            // Before the write, so the watch does not send it back.
            let mut clips = lock_clips(&self.shared);
            clips.received(content.clone(), item);
            clips.settings().show_note
        };
        self.shared.platform().clipboard_write(&content)?;
        self.emit(Event::ClipReceived {
            device: self.device,
            note,
        });
        Ok(())
    }

    fn file(&self, stream: Stream) {
        let offered = Arc::new(OnceLock::<FileOffer>::new());
        let mut handoff = false;
        let mut decide = |offer: &FileOffer| {
            let _ = offered.set(offer.clone());
            if self.take_handoff_file(offer) {
                handoff = true;
                return OfferAnswer::Accept { resume_from: 0 };
            }
            if local(&self.shared, &self.device, Feature::Files) == FeatureState::Off {
                return OfferAnswer::Decline;
            }
            self.ask(AskKind::File {
                name: offer.name.clone(),
                size: offer.size,
            })
        };
        let sink = Box::new(Download::new(
            Arc::clone(self.shared.platform()),
            Arc::clone(&offered),
        ));
        let mut progress = |done| {
            if let Some(o) = offered.get() {
                self.emit(Event::FileProgress {
                    device: self.device,
                    id: o.id,
                    done,
                    total: o.size,
                });
            }
        };
        let received = receive_file(stream, &mut decide, sink, &mut progress);
        let Some(id) = offered.get().map(|o| o.id) else {
            return;
        };
        let device = self.device;
        match received {
            Ok(path) => {
                if handoff {
                    self.open(|p| p.open_path(&path));
                }
                self.emit(Event::FileSaved { device, id, path });
            }
            Err(LinkError::Declined | LinkError::Expired) => {}
            Err(e) => {
                let reason = FileFailure::of_link(&e);
                tracing::debug!(?reason, "file not received");
                self.emit(Event::FileFailed {
                    device,
                    id,
                    reason,
                    retry: false,
                });
            }
        }
    }

    /// Whether `offer` is the file of a handoff accepted on this link
    /// (the id, size and safe name the user accepted; each is taken once).
    /// Any other file is asked about on its own.
    fn take_handoff_file(&self, offer: &FileOffer) -> bool {
        self.with_link(|l| {
            let due = l
                .handoff_files
                .iter()
                .position(|f| (f.id, f.size, &f.name) == (offer.id, offer.size, &offer.name));
            due.map(|i| l.handoff_files.swap_remove(i)).is_some()
        })
        .unwrap_or(false)
    }

    fn handoff(&self, stream: Stream) -> Result<()> {
        let mut decide = |h: &ReceivedHandoff| {
            let kind = match h {
                ReceivedHandoff::Url(url) => AskKind::Handoff {
                    url_host: Some(host(url.as_str()).to_owned()),
                    file: None,
                },
                ReceivedHandoff::File(offer) => AskKind::Handoff {
                    url_host: None,
                    file: Some(offer.name.clone()),
                },
            };
            let answer = self.ask(kind);
            if let (OfferAnswer::Accept { .. }, ReceivedHandoff::File(offer)) = (answer, h) {
                // Recorded before the answer goes out: the file lane follows it.
                self.with_link(|l| l.handoff_files.push(offer.clone()));
            }
            answer
        };
        if let Some(ReceivedHandoff::Url(url)) = recv_handoff(stream, &mut decide)? {
            self.open(|p| p.open_url(url.as_str()));
        }
        Ok(())
    }

    /// Open something handed off ([`Event::HandoffOpened`]).
    fn open(
        &self,
        f: impl FnOnce(&dyn carrova_platform::Platform) -> std::result::Result<(), PlatformError>,
    ) {
        match f(&**self.shared.platform()) {
            Ok(()) => self.emit(Event::HandoffOpened {
                device: self.device,
            }),
            Err(e) => tracing::warn!(error = %e, "could not open a handoff"),
        }
    }

    fn ring(&self, stream: Stream) -> Result<()> {
        let platform = self.shared.platform();
        let device = self.device;
        match recv_ring(stream)? {
            RingMsg::Start { seconds } => {
                platform.ring(seconds)?;
                self.emit(Event::RingStarted { device });
            }
            RingMsg::Stop => {
                platform.stop_ring()?;
                self.emit(Event::RingStopped { device });
            }
        }
        Ok(())
    }
}

/// The host of a checked URL (`scheme://host[:port]/…`, as normalised by
/// [`carrova_link::check_url`]).
fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(v6),
        None => authority.split(':').next().unwrap_or(authority),
    }
}

/// `mime` if it looks like `type/subtype` and is short, else `None`.
pub(super) fn clean_mime(mime: Option<&str>) -> Option<String> {
    let mime = mime?;
    let token = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$&-^_.+".contains(&b))
    };
    let (kind, sub) = mime.split_once('/')?;
    (mime.len() <= MAX_MIME && token(kind) && token(sub)).then(|| mime.to_owned())
}

#[cfg(test)]
#[path = "../../tests/ecosystem/features/unit.rs"]
mod tests;
