//! The process that runs the user's supervisor and serves it on
//! `<state>/link.sock`.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use carrova_link::ReceivedClip;
use carrova_platform::{FileKind, FileLock, LocalListener, LocalStream, Platform, PlatformError};

use super::wire::{
    ControlError, ControlRequest, ControlResponse, MAX_CONTROL_MESSAGE, MAX_CONTROL_REPLY,
    read_message, write_message,
};
use super::{SOCKET_FILE, spawn};
use crate::ecosystem::{HandoffItem, PeerSource, Supervisor, SupervisorConfig};
use crate::engine::Engine;
use crate::error::{CoreError, Result};
use crate::fsx;
use crate::session::{Event, EventSink};

/// First and longest wait before accepting again after a failed accept.
const ACCEPT_BACKOFF_MIN: Duration = Duration::from_millis(100);
const ACCEPT_BACKOFF_MAX: Duration = Duration::from_secs(5);
/// Events a subscriber may fall behind by before it is dropped (emit
/// never waits for one).
pub(super) const SUBSCRIBER_QUEUE: usize = 1024;

/// Runs the user's supervisor and serves it on `<state>/link.sock`.
/// Dropping it hangs up every client and stops the supervisor (after a
/// request still running finishes), then releases the lock.
#[derive(Debug)]
pub struct ControlServer {
    served: Arc<Served>,
    conns: Arc<Conns>,
    stopping: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

/// The supervisor, its socket file and the lock that makes it the user's
/// only one. Dropped in field order: the supervisor stops, the socket file
/// goes, then the lock is released (so the file never outlives the lock
/// and is never removed from under the next server).
#[derive(Debug)]
struct Served {
    supervisor: Supervisor,
    platform: Arc<dyn Platform>,
    events: Arc<Fanout>,
    /// `None` where the OS has no local sockets.
    socket: Option<SocketFile>,
    _lock: FileLock,
}

impl ControlServer {
    /// Start the supervisor while holding `lock` and serve it (replacing a
    /// socket file a dead process left behind). Where the OS has no local
    /// sockets it is only run here: the lock still keeps it the user's
    /// only one.
    pub(super) fn start(
        engine: Engine,
        source: Arc<dyn PeerSource>,
        config: SupervisorConfig,
        lock: FileLock,
    ) -> Result<Self> {
        let platform = Arc::clone(engine.platform());
        let events = Arc::new(Fanout::new(Arc::clone(engine.sink())));
        let engine = engine.with_sink(Arc::clone(&events) as Arc<dyn EventSink>);
        let supervisor = Supervisor::start_with(engine, source, config)?;
        let path = platform.state_dir()?.join(SOCKET_FILE);
        fsx::remove_any(&*platform, &path)?;
        let listener = match platform.bind_local_socket(&path) {
            Ok(listener) => Some(listener),
            Err(PlatformError::Unsupported(_)) => None,
            Err(e) => return Err(e.into()),
        };
        let socket = listener.is_some().then(|| SocketFile {
            platform: Arc::clone(&platform),
            path,
        });
        let served = Arc::new(Served {
            supervisor,
            platform,
            events,
            socket,
            _lock: lock,
        });
        let conns = Arc::<Conns>::default();
        let stopping = Arc::new(AtomicBool::new(false));
        let accept = match listener {
            Some(listener) => {
                let (served, conns) = (Arc::downgrade(&served), Arc::clone(&conns));
                let stopping = Arc::clone(&stopping);
                Some(spawn("carrova-control-accept", move || {
                    accept_loop(&listener, &served, &conns, &stopping);
                })?)
            }
            None => None,
        };
        Ok(Self {
            served,
            conns,
            stopping,
            accept,
        })
    }

    /// The supervisor this process runs.
    #[must_use]
    pub fn supervisor(&self) -> &Supervisor {
        &self.served.supervisor
    }

    pub(super) fn run(&self, request: ControlRequest) -> Result<ControlResponse> {
        self.served.run(request)
    }

    pub(super) fn subscribe(&self) -> Receiver<Event> {
        self.served.events.subscribe()
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        if let (Some(accept), Some(socket)) = (self.accept.take(), &self.served.socket) {
            // Wake the blocked accept; without that it can never be joined.
            match self.served.platform.connect_local_socket(&socket.path) {
                Ok(_) => {
                    if accept.join().is_err() {
                        tracing::warn!("control accept thread panicked");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "control accept thread left running"),
            }
        }
        // Subscribers end; connection threads hold the server only while
        // serving a request, so once hung up `served` drops with this.
        self.served.events.close();
        self.conns.shutdown_all();
    }
}

impl Served {
    /// Run `request` on the supervisor.
    fn run(&self, request: ControlRequest) -> Result<ControlResponse> {
        let (sup, platform) = (&self.supervisor, &*self.platform);
        let done = |()| ControlResponse::Done;
        Ok(match request {
            ControlRequest::Status => ControlResponse::Status(sup.status()),
            ControlRequest::Ping(d) => {
                let rtt = sup.ping(&d)?;
                ControlResponse::Pong(u64::try_from(rtt.as_millis()).unwrap_or(u64::MAX))
            }
            ControlRequest::SendClipboard(d) => sup.send_clipboard(&d).map(done)?,
            ControlRequest::SendFile(d, path) => {
                regular_file(platform, &path)?;
                sup.send_file(&d, &path).map(done)?
            }
            ControlRequest::Handoff(d, item) => {
                if let HandoffItem::File(path) = &item {
                    regular_file(platform, path)?;
                }
                sup.handoff(&d, item).map(done)?
            }
            ControlRequest::Ring(d) => sup.ring(&d).map(done)?,
            ControlRequest::StopRing(d) => sup.stop_ring(&d).map(done)?,
            ControlRequest::Answer(ask, answer) => sup.answer(ask, answer).map(done)?,
            ControlRequest::SetFeature(d, f, s) => sup.set_feature(&d, f, s).map(done)?,
            ControlRequest::ClearClipHistory => {
                sup.clear_clip_history();
                ControlResponse::Done
            }
            ControlRequest::ClipSettings => ControlResponse::ClipSettings(sup.clip_settings()),
            ControlRequest::SetClipSettings(s) => sup.set_clip_settings(s).map(done)?,
            // Answered with more than one message ([`serve_connection`]).
            ControlRequest::ClipHistory | ControlRequest::Subscribe => {
                return Err(ControlError::Unexpected.into());
            }
        })
    }
}

/// A client may name any path: only absolute paths to regular files are
/// sent.
fn regular_file(platform: &dyn Platform, path: &Path) -> Result<()> {
    if path.is_absolute() && platform.metadata(path)?.kind == FileKind::File {
        Ok(())
    } else {
        Err(ControlError::NotAFile.into())
    }
}

/// `<state>/link.sock`, removed on drop.
#[derive(Debug)]
struct SocketFile {
    platform: Arc<dyn Platform>,
    path: PathBuf,
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        if let Err(e) = fsx::remove_any(&*self.platform, &self.path) {
            tracing::warn!(error = %e, "control socket left behind");
        }
    }
}

/// The engine's sink, also copying every event to the subscribers.
struct Fanout {
    inner: Arc<dyn EventSink>,
    subscribers: Mutex<Vec<SyncSender<Event>>>,
}

impl std::fmt::Debug for Fanout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fanout").finish_non_exhaustive()
    }
}

impl Fanout {
    fn new(inner: Arc<dyn EventSink>) -> Self {
        Self {
            inner,
            subscribers: Mutex::default(),
        }
    }

    fn subscribers(&self) -> MutexGuard<'_, Vec<SyncSender<Event>>> {
        self.subscribers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn subscribe(&self) -> Receiver<Event> {
        let (tx, rx) = mpsc::sync_channel(SUBSCRIBER_QUEUE);
        self.subscribers().push(tx);
        rx
    }

    /// End every subscription.
    fn close(&self) {
        self.subscribers().clear();
    }
}

impl EventSink for Fanout {
    fn emit(&self, event: Event) {
        // A full queue (a stalled subscriber) or a dropped receiver ends
        // that subscription.
        self.subscribers()
            .retain(|s| s.try_send(event.clone()).is_ok());
        self.inner.emit(event);
    }
}

/// Open client connections, so a stopping server can hang them up.
#[derive(Debug, Default)]
struct Conns {
    next: AtomicU64,
    open: Mutex<HashMap<u64, LocalStream>>,
}

impl Conns {
    fn open(&self) -> MutexGuard<'_, HashMap<u64, LocalStream>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Hang up every connection: their blocked reads and writes end.
    fn shutdown_all(&self) {
        for stream in self.open().values() {
            // Already closed by the client: nothing to do.
            let _ = stream.shutdown();
        }
    }
}

fn accept_loop(
    listener: &LocalListener,
    served: &Weak<Served>,
    conns: &Arc<Conns>,
    stopping: &AtomicBool,
) {
    let mut backoff = ACCEPT_BACKOFF_MIN;
    loop {
        let accepted = listener.accept();
        if stopping.load(Ordering::SeqCst) {
            return;
        }
        match accepted {
            Ok(stream) => {
                backoff = ACCEPT_BACKOFF_MIN;
                if let Err(e) = start_connection(stream, served, conns) {
                    tracing::warn!(error = %e, "control connection dropped");
                }
            }
            // Another user, or a peer that gave up: wait for the next.
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::PermissionDenied
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::Interrupted
                ) => {}
            // E.g. out of descriptors: retry, as the lock keeps others out.
            Err(e) => {
                tracing::warn!(error = %e, "control socket accept failed; retrying");
                thread::sleep(backoff);
                backoff = (backoff * 2).min(ACCEPT_BACKOFF_MAX);
            }
        }
    }
}

/// Serve `stream` on its own thread (not joined: a request may block for
/// minutes, e.g. a file ask), registered in `conns` while open.
fn start_connection(stream: LocalStream, served: &Weak<Served>, conns: &Arc<Conns>) -> Result<()> {
    let id = conns.next.fetch_add(1, Ordering::Relaxed);
    conns.open().insert(id, stream.try_clone()?);
    let (served, conns_t) = (Weak::clone(served), Arc::clone(conns));
    let spawned = spawn("carrova-control-conn", move || {
        serve_connection(stream, &served);
        conns_t.open().remove(&id);
    });
    if spawned.is_err() {
        conns.open().remove(&id);
    }
    spawned.map(drop)
}

/// Answer requests on `stream` until the client or the server hangs up.
/// A message that cannot be read gets [`ControlResponse::Failed`] and ends
/// the connection.
fn serve_connection(mut stream: LocalStream, served: &Weak<Served>) {
    loop {
        let request = match read_message(&mut stream, MAX_CONTROL_MESSAGE) {
            Ok(Some(request)) => request,
            Ok(None) => return,
            Err(e) => {
                // Not the error itself: a decode error echoes the input.
                tracing::debug!("unreadable control request");
                let failed = ControlResponse::Failed(e.to_string());
                let _ = write_message(&mut stream, &failed, MAX_CONTROL_REPLY);
                return;
            }
        };
        // Held only for the request: an idle client keeps nothing alive.
        let Some(served) = served.upgrade() else {
            return;
        };
        let sent = match request {
            ControlRequest::Subscribe => {
                let events = served.events.subscribe();
                drop(served);
                return stream_events(stream, &events);
            }
            ControlRequest::ClipHistory => {
                let clips = served.supervisor.clip_history();
                drop(served);
                send_clips(&mut stream, &clips)
            }
            request => {
                let response = served
                    .run(request)
                    .unwrap_or_else(|e| ControlResponse::Failed(e.to_string()));
                drop(served);
                reply(&mut stream, &response)
            }
        };
        if let Err(e) = sent {
            tracing::debug!(error = %e, "control client left");
            return;
        }
    }
}

/// Write `response`, or [`ControlResponse::Failed`] if it is over the
/// size limit (nothing of it was written then).
fn reply(stream: &mut LocalStream, response: &ControlResponse) -> Result<()> {
    match write_message(stream, response, MAX_CONTROL_REPLY) {
        Err(CoreError::Control(ControlError::TooLarge)) => {
            let failed = ControlResponse::Failed(ControlError::TooLarge.to_string());
            write_message(stream, &failed, MAX_CONTROL_REPLY)
        }
        written => written,
    }
}

/// The count, then one message per clip. Every clip fits the limit (its
/// size is checked before it enters the history).
fn send_clips(stream: &mut LocalStream, clips: &[ReceivedClip]) -> Result<()> {
    let count = u32::try_from(clips.len()).map_err(|_| ControlError::TooLarge)?;
    reply(stream, &ControlResponse::ClipHistory(count))?;
    clips
        .iter()
        .try_for_each(|clip| write_message(stream, clip, MAX_CONTROL_REPLY))
}

/// Acknowledge a subscription, then send each event until either side
/// ends it.
fn stream_events(mut stream: LocalStream, events: &Receiver<Event>) {
    if write_message(&mut stream, &ControlResponse::Done, MAX_CONTROL_REPLY).is_err() {
        return;
    }
    for event in events {
        if write_message(&mut stream, &event, MAX_CONTROL_REPLY).is_err() {
            return;
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/ecosystem/control/server/unit.rs"]
mod tests;
