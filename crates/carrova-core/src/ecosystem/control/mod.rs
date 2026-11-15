//! One link supervisor per user (spec "All go through the one supervisor
//! per user"): the first process takes `<state>/link.lock`, runs the
//! [`Supervisor`] and serves it on `<state>/link.sock`; later processes
//! become clients of it. Either way a front-end holds a
//! [`SupervisorHandle`], which behaves the same.
//!
//! A client sends a [`ControlRequest`] and reads one [`ControlResponse`],
//! as often as it likes on one connection; [`ControlRequest::Subscribe`]
//! turns a connection into a stream of events.

mod client;
mod server;
mod shared;
mod wire;

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use carrova_crypto::identity::DeviceId;
use carrova_link::{Feature, FeatureState, ReceivedClip};

pub use client::ControlClient;
pub use server::ControlServer;
pub use shared::SharedSupervisor;
pub use wire::{ControlError, ControlRequest, ControlResponse, MAX_CONTROL_MESSAGE};

#[cfg(doc)]
use super::Supervisor;
use super::{
    Answer, AskId, ClipSettings, HandoffItem, LinkedPeer, PeerSource, PresenceSource,
    SupervisorConfig,
};
use crate::engine::Engine;
use crate::error::Result;
use crate::session::Event;
use crate::store::ensure_dir;

/// Held by the process that runs the user's supervisor.
const LOCK_FILE: &str = "link.lock";
/// Where that process serves it.
const SOCKET_FILE: &str = "link.sock";

/// This process's view of the user's supervisor. Every call behaves the
/// same either way; only a remote refusal arrives as
/// [`ControlError::Failed`] (its message) instead of the typed error.
#[derive(Debug)]
pub enum SupervisorHandle {
    /// It runs here (and is served to other processes).
    Local(ControlServer),
    /// It runs in another process.
    Remote(ControlClient),
}

impl SupervisorHandle {
    fn call(&self, request: ControlRequest) -> Result<ControlResponse> {
        match self {
            Self::Local(server) => server.run(request),
            Self::Remote(client) => client.request(&request),
        }
    }

    fn done(&self, request: ControlRequest) -> Result<()> {
        match self.call(request)? {
            ControlResponse::Done => Ok(()),
            _ => Err(ControlError::Unexpected.into()),
        }
    }

    /// [`Supervisor::status`].
    ///
    /// # Errors
    ///
    /// The control socket failed.
    pub fn status(&self) -> Result<Vec<LinkedPeer>> {
        match self.call(ControlRequest::Status)? {
            ControlResponse::Status(peers) => Ok(peers),
            _ => Err(ControlError::Unexpected.into()),
        }
    }

    /// [`Supervisor::ping`], to the millisecond.
    ///
    /// # Errors
    ///
    /// As [`Supervisor::ping`], or the control socket failed.
    pub fn ping(&self, device: &DeviceId) -> Result<Duration> {
        match self.call(ControlRequest::Ping(*device))? {
            ControlResponse::Pong(ms) => Ok(Duration::from_millis(ms)),
            _ => Err(ControlError::Unexpected.into()),
        }
    }

    /// [`Supervisor::send_clipboard`].
    ///
    /// # Errors
    ///
    /// As [`Supervisor::send_clipboard`], or the control socket failed.
    pub fn send_clipboard(&self, device: &DeviceId) -> Result<()> {
        self.done(ControlRequest::SendClipboard(*device))
    }

    /// [`Supervisor::send_text`].
    ///
    /// # Errors
    ///
    /// As [`Supervisor::send_text`], or the control socket failed.
    pub fn send_text(&self, device: &DeviceId, text: String) -> Result<()> {
        self.done(ControlRequest::SendText(*device, text))
    }

    /// [`Supervisor::send_file`]; `path` must be absolute and name a
    /// regular file.
    ///
    /// # Errors
    ///
    /// [`ControlError::NotAFile`], as [`Supervisor::send_file`], or the
    /// control socket failed.
    pub fn send_file(&self, device: &DeviceId, path: &Path) -> Result<()> {
        self.done(ControlRequest::SendFile(*device, path.to_path_buf()))
    }

    /// [`Supervisor::handoff`]; a file as for [`Self::send_file`].
    ///
    /// # Errors
    ///
    /// As [`Self::send_file`] and [`Supervisor::handoff`].
    pub fn handoff(&self, device: &DeviceId, item: HandoffItem) -> Result<()> {
        self.done(ControlRequest::Handoff(*device, item))
    }

    /// [`Supervisor::retry_file`].
    ///
    /// # Errors
    ///
    /// As [`Supervisor::retry_file`], or the control socket failed.
    pub fn retry_file(&self, id: u64) -> Result<()> {
        self.done(ControlRequest::RetryFile(id))
    }

    /// [`Supervisor::ring`].
    ///
    /// # Errors
    ///
    /// As [`Supervisor::ring`], or the control socket failed.
    pub fn ring(&self, device: &DeviceId) -> Result<()> {
        self.done(ControlRequest::Ring(*device))
    }

    /// [`Supervisor::stop_ring`].
    ///
    /// # Errors
    ///
    /// As [`Supervisor::stop_ring`], or the control socket failed.
    pub fn stop_ring(&self, device: &DeviceId) -> Result<()> {
        self.done(ControlRequest::StopRing(*device))
    }

    /// [`Supervisor::answer`].
    ///
    /// # Errors
    ///
    /// As [`Supervisor::answer`], or the control socket failed.
    pub fn answer(&self, ask: AskId, answer: Answer) -> Result<()> {
        self.done(ControlRequest::Answer(ask, answer))
    }

    /// [`Supervisor::set_feature`].
    ///
    /// # Errors
    ///
    /// As [`Supervisor::set_feature`], or the control socket failed.
    pub fn set_feature(
        &self,
        device: &DeviceId,
        feature: Feature,
        state: FeatureState,
    ) -> Result<()> {
        self.done(ControlRequest::SetFeature(*device, feature, state))
    }

    /// [`Supervisor::clip_history`].
    ///
    /// # Errors
    ///
    /// The control socket failed.
    pub fn clip_history(&self) -> Result<Vec<ReceivedClip>> {
        match self {
            Self::Local(server) => Ok(server.supervisor().clip_history()),
            Self::Remote(client) => client.clip_history(),
        }
    }

    /// [`Supervisor::clear_clip_history`].
    ///
    /// # Errors
    ///
    /// The control socket failed.
    pub fn clear_clip_history(&self) -> Result<()> {
        self.done(ControlRequest::ClearClipHistory)
    }

    /// [`Supervisor::clip_settings`].
    ///
    /// # Errors
    ///
    /// The control socket failed.
    pub fn clip_settings(&self) -> Result<ClipSettings> {
        match self.call(ControlRequest::ClipSettings)? {
            ControlResponse::ClipSettings(settings) => Ok(settings),
            _ => Err(ControlError::Unexpected.into()),
        }
    }

    /// [`Supervisor::set_clip_settings`].
    ///
    /// # Errors
    ///
    /// As [`Supervisor::set_clip_settings`], or the control socket failed.
    pub fn set_clip_settings(&self, settings: ClipSettings) -> Result<()> {
        self.done(ControlRequest::SetClipSettings(settings))
    }

    /// The supervisor's events from now on (besides the engine's sink of
    /// the process running it). The receiver ends when it stops, or once
    /// it falls more than 1024 events behind.
    ///
    /// # Errors
    ///
    /// The control socket failed.
    pub fn subscribe(&self) -> Result<Receiver<Event>> {
        match self {
            Self::Local(server) => Ok(server.subscribe()),
            Self::Remote(client) => client.subscribe(),
        }
    }
}

/// [`attach_or_start_with`] watching presence, with the spec's pacing.
///
/// # Errors
///
/// As [`attach_or_start_with`].
pub fn attach_or_start(engine: Engine) -> Result<SupervisorHandle> {
    let source = Arc::new(PresenceSource::new(Arc::clone(engine.platform())));
    attach_or_start_with(engine, source, SupervisorConfig::default())
}

/// Attach to the user's supervisor, or, if no process runs one, start it
/// here with `source` and `config` and serve it (where the OS has local
/// sockets).
///
/// # Errors
///
/// The state directory is unusable, the supervisor could not start or its
/// socket could not be bound, or a running one could not be reached.
pub fn attach_or_start_with(
    engine: Engine,
    source: Arc<dyn PeerSource>,
    config: SupervisorConfig,
) -> Result<SupervisorHandle> {
    let platform = Arc::clone(engine.platform());
    let dir = platform.state_dir()?;
    ensure_dir(&*platform, &dir, 0o700)?;
    // Lock first: a client must never start (and on drop clear the port
    // file of) a second supervisor.
    match platform.try_lock_exclusive(&dir.join(LOCK_FILE))? {
        None => ControlClient::connect(&platform).map(SupervisorHandle::Remote),
        Some(lock) => {
            ControlServer::start(engine, source, config, lock).map(SupervisorHandle::Local)
        }
    }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> Result<JoinHandle<()>> {
    Ok(thread::Builder::new().name(name.to_owned()).spawn(f)?)
}
