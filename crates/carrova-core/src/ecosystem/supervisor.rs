//! The link supervisor (spec "The link"): keeps known devices linked while
//! a feature is on for them and carries clipboard, files, handoff, ring and
//! (from a phone) notifications, SMS and calls over their links. Runs on
//! plain threads (the transport and link calls block, and must not run
//! inside an async runtime).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use carrova_crypto::identity::{DeviceId, DeviceKey};
use carrova_link::{
    ActionResult, CallCommand, CallReply, Feature, FeatureState, FileOffer, Handoff, NotifyAction,
    OfferAnswer, PolicyDoc, RING_SECONDS, ReceivedClip, RingMsg, SmsReply, SmsRequest, allowed,
    call_command, offer_file, request_sms, send_action, send_handoff, send_ring,
};
use carrova_platform::phone::Note;
use carrova_platform::{ClipContent, ClipKind, ClipboardWatch, Platform};
use carrova_transport::Connection;

use super::asks::{Answer, AskId, Asks};
use super::clip::{self, Clips};
use super::clip_settings::ClipSettings;
use super::features::{self, FileFailure};
use super::peer::{Link, LinkedPeer, Peer, PeerSource};
use super::phone::give::{self, Gives, Job};
use super::phone::settings::{self, PhoneSettings};
use super::phone::{calls, mirror};
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
    /// The user's settings for showing a linked phone here.
    phone: Mutex<PhoneSettings>,
    /// The phone forwarder's queue ([`give::run`]).
    pub(super) give: Sender<Job>,
    /// Fires on [`Supervisor::stop`]: every thread and connection ends.
    pub(super) stop: Cancel,
    /// Per-link, lane and dial threads, joined on stop.
    threads: Mutex<Vec<JoinHandle<()>>>,
    /// Held from a call view's change to its emit, so views show in the
    /// order they changed. Taken before `peers`, never across a platform call.
    calls: Mutex<()>,
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

    /// Serialise call view changes with their emits (see `calls`).
    pub(super) fn call_views(&self) -> MutexGuard<'_, ()> {
        self.calls.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn phone_settings(&self) -> PhoneSettings {
        *self.phone.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn platform(&self) -> &Arc<dyn Platform> {
        self.engine.platform()
    }

    pub(super) fn emit(&self, event: Event) {
        self.engine.sink().emit(event);
    }

    /// The linked devices `feature` is on for, on both sides, and their
    /// connections.
    pub(super) fn linked_with(&self, feature: Feature) -> Vec<(DeviceId, Arc<Connection>)> {
        self.peers()
            .iter()
            .filter_map(|p| {
                let link = p.link.as_ref()?;
                let on = allowed(&p.local, link.remote.as_ref()?, feature);
                (on == FeatureState::On).then(|| (p.id(), Arc::clone(&link.conn)))
            })
            .collect()
    }

    /// Run `f` on `device`'s link `generation`, if it is still the live one.
    pub(super) fn with_link<T>(
        &self,
        device: &DeviceId,
        generation: u64,
        f: impl FnOnce(&mut Link) -> T,
    ) -> Option<T> {
        let mut peers = self.peers();
        let link = peers
            .iter_mut()
            .find(|p| p.id() == *device)?
            .link
            .as_mut()
            .filter(|l| l.generation == generation)?;
        Some(f(link))
    }

    /// `device`'s live connection, if `feature` is allowed on both sides
    /// (`None`: any feature).
    fn conn(&self, device: &DeviceId, feature: Option<Feature>) -> Result<Arc<Connection>> {
        self.live(device, feature).map(|(conn, _)| conn)
    }

    /// As [`Self::conn`], with the link's generation.
    fn live(&self, device: &DeviceId, feature: Option<Feature>) -> Result<(Arc<Connection>, u64)> {
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
        Ok((Arc::clone(&link.conn), link.generation))
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
        let (give, jobs) = mpsc::channel();
        let shared = Arc::new(Shared {
            engine,
            config,
            me: key.public(),
            key,
            peers: Mutex::default(),
            asks: Asks::default(),
            clips: Mutex::new(Clips::load(Arc::clone(&platform))),
            phone: Mutex::new(settings::load(&*platform).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "phone settings unreadable; using the defaults");
                PhoneSettings::default()
            })),
            give,
            stop: Cancel::default(),
            threads: Mutex::default(),
            calls: Mutex::default(),
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
        // Joined after the links close: a send in progress ends with them.
        let shared = Arc::clone(&sup.shared);
        sup.shared.spawn("carrova-phone-give", move || {
            give::run(&shared, &jobs);
        })?;
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

    /// The notifications `device`'s phone shows, newest first (none while
    /// it is not linked). Held in memory only; while this computer is (or
    /// may be) locked they carry no content, as in [`Event::PhoneNote`].
    ///
    /// # Errors
    ///
    /// [`CoreError::UnknownDevice`].
    pub fn notes(&self, device: &DeviceId) -> Result<Vec<Note>> {
        let notes = {
            let peers = self.shared.peers();
            let peer = peers.iter().find(|p| p.id() == *device);
            let peer = peer.ok_or(CoreError::UnknownDevice)?;
            peer.link
                .as_ref()
                .map(|l| l.mirror.notes())
                .unwrap_or_default()
        };
        Ok(mirror::shown_now(&self.shared, notes))
    }

    /// Carry out `action` on a notification of `device`'s phone (reply,
    /// dismiss, open there).
    ///
    /// # Errors
    ///
    /// [`CoreError::NotLinked`] (the phone is away),
    /// [`CoreError::FeatureOff`], or the link failed.
    pub fn note_action(&self, device: &DeviceId, action: &NotifyAction) -> Result<ActionResult> {
        let conn = self.shared.conn(device, Some(Feature::Notifications))?;
        Ok(send_action(&conn, action)?)
    }

    /// Ask `device`'s phone for SMS threads or messages, or to send a text
    /// (sent at most once per `client_id`). The newest page of a thread
    /// also holds its texts the phone has not stored yet (`id` 0).
    ///
    /// # Errors
    ///
    /// [`CoreError::NotLinked`] (the phone is away: nothing is sent),
    /// [`CoreError::FeatureOff`], or the link failed.
    pub fn sms(&self, device: &DeviceId, request: &SmsRequest) -> Result<SmsReply> {
        let (conn, generation) = self.shared.live(device, Some(Feature::Messages))?;
        let reply = request_sms(&conn, request)?;
        Ok(mirror::take_sms_reply(
            &self.shared,
            *device,
            generation,
            request,
            reply,
        ))
    }

    /// Carry out `command` on `device`'s phone call: keypad tones and the
    /// audio route here, on this computer's call audio; the rest on the
    /// phone.
    ///
    /// # Errors
    ///
    /// [`CoreError::NotLinked`], [`CoreError::FeatureOff`] or the link
    /// failed.
    pub fn call(&self, device: &DeviceId, command: &CallCommand) -> Result<CallReply> {
        let (conn, generation) = self.shared.live(device, Some(Feature::Calls))?;
        let platform = &**self.shared.platform();
        let done = |r: std::result::Result<(), carrova_platform::PlatformError>| match r {
            Ok(()) => CallReply::Done,
            Err(e) => {
                tracing::debug!(?command, error = %e, "call command failed here");
                CallReply::Failed
            }
        };
        match command {
            CallCommand::Tones(digits) => Ok(done(platform.call_tones(digits))),
            CallCommand::AudioTo(side) => {
                let reply = done(platform.call_audio_to(*side));
                if reply == CallReply::Done {
                    calls::reread_audio(&self.shared, *device, generation, false);
                }
                Ok(reply)
            }
            _ => Ok(call_command(&conn, command)?),
        }
    }

    /// The user's settings for showing a linked phone here.
    #[must_use]
    pub fn phone_settings(&self) -> PhoneSettings {
        self.shared.phone_settings()
    }

    /// Save `settings` and apply them from now on.
    ///
    /// # Errors
    ///
    /// The settings could not be written.
    pub fn set_phone_settings(&self, settings: PhoneSettings) -> Result<()> {
        let mut current = self
            .shared
            .phone
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        settings::save(&**self.shared.platform(), &settings)?;
        *current = settings;
        Ok(())
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

    /// Send `text` (shared with this app, not copied) to `device` as a
    /// clip, as [`Self::send_clipboard`] would; this device's clipboard is
    /// left alone. Over [`carrova_link::MAX_CLIP_TEXT`] it is skipped
    /// ([`Event::ClipSkipped`]).
    ///
    /// # Errors
    ///
    /// [`CoreError::NotLinked`], [`CoreError::FeatureOff`], or the link
    /// failed.
    pub fn send_text(&self, device: &DeviceId, text: String) -> Result<()> {
        let conn = self.shared.conn(device, Some(Feature::Clipboard))?;
        let content = ClipContent {
            kind: ClipKind::Text,
            bytes: text.into_bytes(),
            sensitive: false,
        };
        match clip::clip_item(&self.shared, content) {
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
        let (conn, gives, cleared) = {
            let mut peers = self.shared.peers();
            let peer = peers.iter_mut().find(|p| p.id() == *device);
            let peer = peer.ok_or(CoreError::UnknownDevice)?;
            let remote = peer.link.as_ref().and_then(|l| l.remote.as_ref());
            let gives = Gives::of(&local, remote).since(Gives::of(&peer.local, remote));
            let cleared = match peer.link.as_mut() {
                Some(l) => l.mirror.follow_policy(&local, l.remote.as_ref()),
                None => Vec::new(),
            };
            peer.local = local;
            (
                peer.link.as_ref().map(|l| Arc::clone(&l.conn)),
                gives,
                cleared,
            )
        };
        mirror::cleared(&self.shared, *device, cleared);
        if let Some(conn) = conn {
            features::push_policy(&self.shared, device, &conn);
        }
        gives.send(&self.shared, *device);
        Ok(())
    }

    /// Offer `path` to `device` (after `announce` succeeds with the
    /// offer), reporting progress and the outcome ([`Event::FileSent`] or
    /// [`Event::FileFailed`]). If `retryable` and the
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
        if sent.is_ok() {
            self.shared.emit(Event::FileSent {
                device: *device,
                id,
            });
        }
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
