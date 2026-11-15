//! The link supervisor (spec "The link"): keeps known devices linked while
//! a feature is on for them and carries clipboard, files, handoff and ring
//! over their links. Runs on plain threads (the transport and link calls
//! block, and must not run inside an async runtime).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use carrova_crypto::identity::{DeviceId, DeviceKey};
use carrova_link::{
    Feature, FeatureState, FileOffer, Handoff, OfferAnswer, PolicyDoc, RING_SECONDS, ReceivedClip,
    RingMsg, allowed, offer_file, send_handoff, send_ring,
};
use carrova_platform::{ClipboardWatch, Platform};
use carrova_transport::Connection;

use super::asks::{Answer, AskId, Asks};
use super::clip::{self, Clips};
use super::clip_settings::ClipSettings;
use super::features::{self, FileFailure};
use super::peer::{LinkedPeer, Peer, PeerSource};
use super::retry::Retries;
use super::{PolicyStore, dial};
use crate::cancel::Cancel;
use crate::devices::KnownDevices;
use crate::engine::Engine;
use crate::error::{CoreError, Result};
use crate::presence::save_link_port;
use crate::session::Event;

/// How the supervisor paces itself. [`Default`] holds the spec's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisorConfig {
    /// How often it reads the known devices and presence, and dials.
    pub tick: Duration,
    /// First wait before redialling a device after a failure.
    pub backoff_min: Duration,
    /// Longest wait between dials (the wait doubles up to it).
    pub backoff_max: Duration,
    /// A device presence has not seen for this long is closed and not
    /// dialled.
    pub away_after: Duration,
    /// How long an ask waits for the user's answer.
    pub ask_expiry: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            tick: Duration::from_secs(1),
            backoff_min: Duration::from_secs(1),
            backoff_max: Duration::from_secs(60),
            away_after: Duration::from_secs(60),
            ask_expiry: Duration::from_secs(60),
        }
    }
}

/// What to hand off to a device ([`Supervisor::handoff`]).
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HandoffItem {
    /// A web link (`http`/`https`) to open there.
    Url(String),
    /// A file to save and open there.
    File(PathBuf),
}

impl std::fmt::Debug for HandoffItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Url(_) => "HandoffItem::Url(..)",
            Self::File(_) => "HandoffItem::File(..)",
        })
    }
}

/// State the supervisor's threads share.
pub(super) struct Shared {
    pub(super) engine: Engine,
    pub(super) config: SupervisorConfig,
    pub(super) me: DeviceId,
    pub(super) key: Arc<DeviceKey>,
    peers: Mutex<Vec<Peer>>,
    pub(super) asks: Asks,
    pub(super) clips: Mutex<Clips>,
    /// Fires on [`Supervisor::stop`]: every thread and connection ends.
    pub(super) stop: Cancel,
    /// Per-link, lane and dial threads, joined on stop.
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Shared {
    /// The known devices, most recently used first. Never held across a
    /// blocking call.
    pub(super) fn peers(&self) -> MutexGuard<'_, Vec<Peer>> {
        self.peers.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Run `f` on a thread the stop waits for.
    pub(super) fn spawn(&self, name: &str, f: impl FnOnce() + Send + 'static) -> Result<()> {
        let thread = spawn(name, f)?;
        let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
        threads.retain(|t| !t.is_finished());
        threads.push(thread);
        Ok(())
    }

    /// Wait for every thread of [`Self::spawn`], including any they start.
    fn join_all(&self) {
        loop {
            let threads =
                std::mem::take(&mut *self.threads.lock().unwrap_or_else(PoisonError::into_inner));
            if threads.is_empty() {
                return;
            }
            join(threads);
        }
    }

    pub(super) fn platform(&self) -> &Arc<dyn Platform> {
        self.engine.platform()
    }

    pub(super) fn emit(&self, event: Event) {
        self.engine.sink().emit(event);
    }

    /// `device`'s live connection, if `feature` is allowed on both sides
    /// (`None`: any feature).
    fn conn(&self, device: &DeviceId, feature: Option<Feature>) -> Result<Arc<Connection>> {
        let peers = self.peers();
        let peer = peers
            .iter()
            .find(|p| p.id() == *device)
            .ok_or(CoreError::UnknownDevice)?;
        let link = peer.link.as_ref().ok_or(CoreError::NotLinked)?;
        if let Some(f) = feature {
            let off = PolicyDoc::computer_defaults();
            let remote = link.remote.as_ref().unwrap_or(&off);
            if allowed(&peer.local, remote, f) == FeatureState::Off {
                return Err(CoreError::FeatureOff(f));
            }
        }
        Ok(Arc::clone(&link.conn))
    }
}

/// The running supervisor. [`Supervisor::stop`] (or drop) ends its links.
#[must_use = "the supervisor stops when dropped"]
pub struct Supervisor {
    shared: Arc<Shared>,
    port: u16,
    /// In a mutex only to make the supervisor `Sync` (the watch is not).
    watch: Mutex<Option<ClipboardWatch>>,
    /// Files whose device left mid-send ([`Self::retry_file`]).
    retries: Mutex<Retries>,
    threads: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for Supervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Supervisor")
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

impl Supervisor {
    /// Start with the spec's pacing ([`SupervisorConfig::default`]).
    ///
    /// # Errors
    ///
    /// The device key or known devices are unreadable, or the link
    /// listener could not bind.
    pub fn start(engine: Engine, source: Arc<dyn PeerSource>) -> Result<Self> {
        Self::start_with(engine, source, SupervisorConfig::default())
    }

    /// Start listening for links, dialling the devices `source` reports and
    /// watching the clipboard (where the platform can).
    ///
    /// # Errors
    ///
    /// As [`Self::start`].
    pub fn start_with(
        engine: Engine,
        source: Arc<dyn PeerSource>,
        config: SupervisorConfig,
    ) -> Result<Self> {
        let platform = Arc::clone(engine.platform());
        let key = Arc::new(KnownDevices::open(&*platform)?.device_key(&*platform)?);
        let shared = Arc::new(Shared {
            engine,
            config,
            me: key.public(),
            key,
            peers: Mutex::default(),
            asks: Asks::default(),
            clips: Mutex::new(Clips::load(Arc::clone(&platform))),
            stop: Cancel::default(),
            threads: Mutex::default(),
        });
        dial::refresh(&shared)?;
        let listener = dial::bind(&shared)?;
        let mut sup = Self {
            port: listener.port(),
            shared,
            watch: Mutex::default(),
            retries: Mutex::default(),
            threads: Vec::new(),
        };
        // From here a failure drops `sup`, which stops what already runs.
        let shared = Arc::clone(&sup.shared);
        sup.threads.push(spawn("carrova-link-accept", move || {
            dial::accept_loop(&shared, listener);
        })?);
        let shared = Arc::clone(&sup.shared);
        sup.threads.push(spawn("carrova-link-dial", move || {
            dial::maintain(&shared, &*source);
        })?);
        if let Some((watch, thread)) = clip::watch_clipboard(&sup.shared)? {
            sup.watch = Mutex::new(Some(watch));
            sup.threads.push(thread);
        }
        save_link_port(&*platform, Some(sup.port))?;
        Ok(sup)
    }

    /// The link listener's UDP port (for the presence advert's `lp`).
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Close every link and wait for every thread of the supervisor: once
    /// it returns, nothing more is received, written or emitted.
    pub fn stop(self) {
        drop(self);
    }

    /// Every known device and its link.
    #[must_use]
    pub fn status(&self) -> Vec<LinkedPeer> {
        self.shared.peers().iter().map(Peer::status).collect()
    }

    /// The last clips received (newest first).
    #[must_use]
    pub fn clip_history(&self) -> Vec<ReceivedClip> {
        clip::lock_clips(&self.shared).history()
    }

    /// Forget the clips received (and their file, if kept).
    pub fn clear_clip_history(&self) {
        clip::lock_clips(&self.shared).clear();
    }

    /// The user's clipboard settings.
    #[must_use]
    pub fn clip_settings(&self) -> ClipSettings {
        clip::lock_clips(&self.shared).settings()
    }

    /// Save `settings` and apply them from now on (the history file is
    /// written or deleted at once).
    ///
    /// # Errors
    ///
    /// The settings or the history file could not be written or deleted.
    pub fn set_clip_settings(&self, settings: ClipSettings) -> Result<()> {
        clip::lock_clips(&self.shared).set_settings(settings)
    }

    /// Round-trip time to `device`.
    ///
    /// # Errors
    ///
    /// [`CoreError::NotLinked`], or the link failed.
    pub fn ping(&self, device: &DeviceId) -> Result<Duration> {
        let conn = self.shared.conn(device, None)?;
        Ok(carrova_link::ping(&conn)?)
    }

    /// Send this clipboard to `device`. Content the OS marks sensitive, or
    /// over the size limits, is not sent ([`Event::ClipSkipped`]).
    ///
    /// # Errors
    ///
    /// [`CoreError::NotLinked`], [`CoreError::FeatureOff`], the clipboard
    /// is unreadable, or the link failed.
    pub fn send_clipboard(&self, device: &DeviceId) -> Result<()> {
        let conn = self.shared.conn(device, Some(Feature::Clipboard))?;
        let content = self.shared.platform().clipboard_read()?;
        match content.and_then(|c| clip::clip_item(&self.shared, c)) {
            Some(item) => clip::send_clip_to(&self.shared, device, &conn, &item),
            None => Ok(()),
        }
    }

    /// Offer the file at `path` to `device` and send it once its user
    /// accepts. Returns when it was saved there.
    ///
    /// # Errors
    ///
    /// [`CoreError::NotLinked`], [`CoreError::FeatureOff`], the file is
    /// unreadable, [`carrova_link::LinkError::Declined`] /
    /// [`carrova_link::LinkError::Expired`], or the link failed
    /// ([`Event::FileFailed`]).
    pub fn send_file(&self, device: &DeviceId, path: &Path) -> Result<()> {
        let conn = self.shared.conn(device, Some(Feature::Files))?;
        self.offer(device, &conn, path, true, |_| Ok(()))
    }

    /// Send file `id` again, from the start (as a new offer, with a new
    /// id): one whose device left mid-send ([`Event::FileFailed`] with
    /// `retry`).
    ///
    /// # Errors
    ///
    /// [`CoreError::NothingToRetry`] if `id` is not such a file (or was
    /// already sent again), else as [`Self::send_file`]. While the device
    /// is not linked the file stays ready to retry.
    pub fn retry_file(&self, id: u64) -> Result<()> {
        let (device, path) = self.retries().take(id).ok_or(CoreError::NothingToRetry)?;
        let conn = match self.shared.conn(&device, Some(Feature::Files)) {
            Ok(conn) => conn,
            Err(e) => {
                self.retries().keep(id, device, path);
                return Err(e);
            }
        };
        self.offer(&device, &conn, &path, true, |_| Ok(()))
    }

    fn retries(&self) -> MutexGuard<'_, Retries> {
        self.retries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hand `item` to `device`: its user is asked, then the link opens there
    /// (a file is sent first, then opened).
    ///
    /// # Errors
    ///
    /// As [`Self::send_file`]; a URL must be `http(s)`
    /// ([`carrova_link::LinkError::UnsafeUrl`]).
    pub fn handoff(&self, device: &DeviceId, item: HandoffItem) -> Result<()> {
        let conn = self.shared.conn(device, Some(Feature::Handoff))?;
        match item {
            HandoffItem::Url(url) => accepted(send_handoff(&conn, &Handoff::Url(url))?),
            HandoffItem::File(path) => self.offer(device, &conn, &path, false, |offer| {
                accepted(send_handoff(&conn, &Handoff::File(offer.clone()))?)
            }),
        }
    }

    /// Make `device` ring for [`RING_SECONDS`].
    ///
    /// # Errors
    ///
    /// [`CoreError::NotLinked`], [`CoreError::FeatureOff`], or the link
    /// failed.
    pub fn ring(&self, device: &DeviceId) -> Result<()> {
        let conn = self.shared.conn(device, Some(Feature::Ring))?;
        let seconds = RING_SECONDS;
        Ok(send_ring(&conn, RingMsg::Start { seconds })?)
    }

    /// Stop `device` ringing.
    ///
    /// # Errors
    ///
    /// As [`Self::ring`].
    pub fn stop_ring(&self, device: &DeviceId) -> Result<()> {
        let conn = self.shared.conn(device, Some(Feature::Ring))?;
        Ok(send_ring(&conn, RingMsg::Stop)?)
    }

    /// Answer an [`Event::AskReceived`].
    ///
    /// # Errors
    ///
    /// [`CoreError::AskGone`] once it was answered or expired.
    pub fn answer(&self, ask: AskId, answer: Answer) -> Result<()> {
        self.shared.asks.answer(ask, answer)
    }

    /// Set `feature` for `device`: saved, and sent to it at once if linked
    /// (the link opens or closes on the next round as needed).
    ///
    /// # Errors
    ///
    /// [`CoreError::UnknownDevice`], a state the feature does not take
    /// ([`CoreError::Link`]), or the policy could not be saved.
    pub fn set_feature(
        &self,
        device: &DeviceId,
        feature: Feature,
        state: FeatureState,
    ) -> Result<()> {
        let platform = &**self.shared.platform();
        if !self.shared.peers().iter().any(|p| p.id() == *device) {
            return Err(CoreError::UnknownDevice);
        }
        let local = PolicyStore::open(platform)?.set(platform, device, feature, state)?;
        let conn = {
            let mut peers = self.shared.peers();
            let peer = peers.iter_mut().find(|p| p.id() == *device);
            let peer = peer.ok_or(CoreError::UnknownDevice)?;
            peer.local = local;
            peer.link.as_ref().map(|l| Arc::clone(&l.conn))
        };
        if let Some(conn) = conn {
            features::push_policy(&self.shared, device, &conn);
        }
        Ok(())
    }

    /// Offer `path` to `device` (after `announce` succeeds with the
    /// offer), reporting progress and failure. If `retryable` and the
    /// device left, it is kept for [`Self::retry_file`].
    fn offer(
        &self,
        device: &DeviceId,
        conn: &Connection,
        path: &Path,
        retryable: bool,
        announce: impl FnOnce(&FileOffer) -> Result<()>,
    ) -> Result<()> {
        let platform = &**self.shared.platform();
        let mut src = platform.open_read_nofollow(path)?;
        let offer = FileOffer {
            id: features::offer_id()?,
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            size: platform.file_metadata(&src)?.size,
            mime: None,
        };
        let (id, total) = (offer.id, offer.size);
        let sent = announce(&offer).and_then(|()| {
            let mut progress = |done| {
                self.shared.emit(Event::FileProgress {
                    device: *device,
                    id,
                    done,
                    total,
                });
            };
            Ok(offer_file(conn, &offer, &mut src, &mut progress)?)
        });
        if let Err(e) = &sent {
            let reason = FileFailure::of(e);
            let retry = retryable && reason == FileFailure::DeviceLeft;
            if retry {
                self.retries().keep(id, *device, path.to_path_buf());
            }
            self.shared.emit(Event::FileFailed {
                device: *device,
                id,
                reason,
                retry,
            });
        }
        sent
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        if let Err(e) = save_link_port(&**self.shared.platform(), None) {
            tracing::warn!(error = %e, "stale link port left for presence");
        }
        self.shared.stop.cancel();
        // Ends the clipboard thread (its channel closes).
        self.watch
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        join(self.threads.drain(..));
        // Only once the listener is gone: a peer that sees its link drop
        // redials at once, and must not reach this stopping supervisor.
        dial::close_all(&self.shared);
        self.shared.join_all();
    }
}

fn join(threads: impl IntoIterator<Item = JoinHandle<()>>) {
    for thread in threads {
        if thread.join().is_err() {
            tracing::warn!("a link supervisor thread panicked");
        }
    }
}

/// `Ok` if the user accepted, else why not.
fn accepted(answer: OfferAnswer) -> Result<()> {
    match answer {
        OfferAnswer::Accept { .. } => Ok(()),
        OfferAnswer::Decline => Err(carrova_link::LinkError::Declined.into()),
        OfferAnswer::Expired => Err(carrova_link::LinkError::Expired.into()),
    }
}

/// Spawn a named thread.
pub(super) fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> Result<JoinHandle<()>> {
    Ok(thread::Builder::new().name(name.to_owned()).spawn(f)?)
}
