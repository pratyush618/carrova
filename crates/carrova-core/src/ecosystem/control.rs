//! One link supervisor per user (spec "All go through the one supervisor
//! per user"): the first process takes `<state>/link.lock`, runs the
//! [`Supervisor`] and serves it on `<state>/link.sock`; later processes
//! become [`ControlClient`]s of it.
//!
//! Wire: each message is a big-endian `u32` length, then that many bytes
//! of CBOR, at most [`MAX_CONTROL_MESSAGE`]. A client sends a
//! [`ControlRequest`] and reads one [`ControlResponse`], as often as it
//! likes on one connection.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use carrova_crypto::identity::DeviceId;
use carrova_link::{Feature, FeatureState};
use carrova_platform::{FileKind, FileLock, LocalListener, LocalStream, Platform};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{HandoffItem, LinkedPeer, PresenceSource, Supervisor};
use crate::engine::Engine;
use crate::error::Result;
use crate::fsx;
use crate::store::ensure_dir;

/// Largest control message body, in bytes.
pub const MAX_CONTROL_MESSAGE: u32 = 64 * 1024;

/// Held by the process that runs the user's supervisor.
const LOCK_FILE: &str = "link.lock";
/// Where that process serves it.
const SOCKET_FILE: &str = "link.sock";
/// How long a client waits for a just-started supervisor to listen.
const CONNECT_WAIT: Duration = Duration::from_secs(5);
/// First and longest wait before accepting again after a failed accept.
const ACCEPT_BACKOFF_MIN: Duration = Duration::from_millis(100);
const ACCEPT_BACKOFF_MAX: Duration = Duration::from_secs(5);

/// What a client asks the supervisor (its [`Supervisor`] call of the same
/// name). Its `Debug` leaves out paths.
#[derive(Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum ControlRequest {
    /// [`Supervisor::status`].
    Status,
    /// [`Supervisor::ping`].
    Ping(DeviceId),
    /// [`Supervisor::send_clipboard`].
    SendClipboard(DeviceId),
    /// [`Supervisor::send_file`]; the path must be absolute and name a
    /// regular file (not a symlink).
    SendFile(DeviceId, PathBuf),
    /// [`Supervisor::handoff`]; a file as for [`Self::SendFile`].
    Handoff(DeviceId, HandoffItem),
    /// [`Supervisor::ring`].
    Ring(DeviceId),
    /// [`Supervisor::set_feature`].
    SetFeature(DeviceId, Feature, FeatureState),
}

impl std::fmt::Debug for ControlRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Status => f.write_str("Status"),
            Self::Ping(d) => f.debug_tuple("Ping").field(d).finish(),
            Self::SendClipboard(d) => f.debug_tuple("SendClipboard").field(d).finish(),
            Self::SendFile(d, _) => f.debug_tuple("SendFile").field(d).finish_non_exhaustive(),
            Self::Handoff(d, item) => f.debug_tuple("Handoff").field(d).field(item).finish(),
            Self::Ring(d) => f.debug_tuple("Ring").field(d).finish(),
            Self::SetFeature(d, feature, state) => f
                .debug_tuple("SetFeature")
                .field(d)
                .field(feature)
                .field(state)
                .finish(),
        }
    }
}

/// The supervisor's answer to a [`ControlRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum ControlResponse {
    /// To [`ControlRequest::Status`].
    Status(Vec<LinkedPeer>),
    /// To [`ControlRequest::Ping`]: the round trip in milliseconds.
    Pong(u64),
    /// The request succeeded.
    Done,
    /// The request failed; the error's message.
    Failed(String),
}

/// Why the control socket failed ([`crate::CoreError::Control`]).
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    /// A message is over [`MAX_CONTROL_MESSAGE`].
    #[error("message over the 64 KiB limit")]
    TooLarge,
    /// A message could not be decoded.
    #[error("unreadable message: {0}")]
    Decode(#[from] ciborium::de::Error<io::Error>),
    /// A message could not be encoded.
    #[error("cannot encode message: {0}")]
    Encode(#[from] ciborium::ser::Error<io::Error>),
    /// The supervisor closed the connection without answering.
    #[error("the supervisor closed the connection")]
    Closed,
    /// A file to send is not an absolute path to a regular file.
    #[error("not an absolute path to a regular file")]
    NotAFile,
}

/// This process's view of the user's supervisor.
#[derive(Debug)]
pub enum SupervisorHandle {
    /// It runs here (and is served to other processes).
    Local(ControlServer),
    /// It runs in another process.
    Remote(ControlClient),
}

impl SupervisorHandle {
    /// Run `request` on the supervisor, wherever it runs.
    ///
    /// # Errors
    ///
    /// Remote: the control socket failed. A failed request is
    /// [`ControlResponse::Failed`].
    pub fn request(&self, request: ControlRequest) -> Result<ControlResponse> {
        match self {
            Self::Local(server) => Ok(server.served.serve(request)),
            Self::Remote(client) => client.request(&request),
        }
    }
}

/// Attach to the user's supervisor, or start it here (watching presence,
/// default pacing) and serve it if no process runs one.
///
/// # Errors
///
/// The state directory is unusable, the supervisor could not start or its
/// socket could not be bound, or a running one could not be reached.
pub fn attach_or_start(engine: Engine) -> Result<SupervisorHandle> {
    let platform = Arc::clone(engine.platform());
    let dir = platform.state_dir()?;
    ensure_dir(&*platform, &dir, 0o700)?;
    // Lock first: a client must never start (and on drop clear the port
    // file of) a second supervisor.
    let Some(lock) = platform.try_lock_exclusive(&dir.join(LOCK_FILE))? else {
        return ControlClient::connect(&*platform).map(SupervisorHandle::Remote);
    };
    let source = Arc::new(PresenceSource::new(Arc::clone(&platform)));
    let supervisor = Supervisor::start(engine, source)?;
    ControlServer::serve(&platform, supervisor, lock).map(SupervisorHandle::Local)
}

/// The supervisor, its socket file and the lock that makes it the user's
/// only one. Dropped in field order: the supervisor stops, the socket file
/// goes, then the lock is released (so the file never outlives the lock
/// and is never removed from under the next server).
#[derive(Debug)]
struct Served {
    supervisor: Supervisor,
    platform: Arc<dyn Platform>,
    socket: SocketFile,
    _lock: FileLock,
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

    /// Hang up every connection: their blocked reads end.
    fn shutdown_all(&self) {
        for stream in self.open().values() {
            // Already closed by the client: nothing to do.
            let _ = stream.shutdown();
        }
    }
}

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

impl ControlServer {
    /// Serve `supervisor` while holding `lock` (replacing a socket file a
    /// dead process left behind).
    fn serve(platform: &Arc<dyn Platform>, supervisor: Supervisor, lock: FileLock) -> Result<Self> {
        let path = platform.state_dir()?.join(SOCKET_FILE);
        fsx::remove_any(&**platform, &path)?;
        let listener = platform.bind_local_socket(&path)?;
        let served = Arc::new(Served {
            supervisor,
            platform: Arc::clone(platform),
            socket: SocketFile {
                platform: Arc::clone(platform),
                path,
            },
            _lock: lock,
        });
        let conns = Arc::<Conns>::default();
        let stopping = Arc::new(AtomicBool::new(false));
        let accept = {
            let (served, conns) = (Arc::downgrade(&served), Arc::clone(&conns));
            let stopping = Arc::clone(&stopping);
            spawn("carrova-control-accept", move || {
                accept_loop(&listener, &served, &conns, &stopping);
            })?
        };
        Ok(Self {
            served,
            conns,
            stopping,
            accept: Some(accept),
        })
    }

    /// The supervisor this process runs.
    #[must_use]
    pub fn supervisor(&self) -> &Supervisor {
        &self.served.supervisor
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        // Wake the blocked accept; without that it can never be joined.
        let socket = &self.served.socket.path;
        match self.served.platform.connect_local_socket(socket) {
            Ok(_) => {
                if let Some(t) = self.accept.take()
                    && t.join().is_err()
                {
                    tracing::warn!("control accept thread panicked");
                }
            }
            Err(e) => tracing::warn!(error = %e, "control accept thread left running"),
        }
        // Connection threads hold the server only while serving a request,
        // so once they are hung up `served` drops with this.
        self.conns.shutdown_all();
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
        let request = match read_message(&mut stream) {
            Ok(Some(request)) => request,
            Ok(None) => return,
            Err(e) => {
                // Not the error itself: a decode error echoes the input.
                tracing::debug!("unreadable control request");
                let _ = write_message(&mut stream, &ControlResponse::Failed(e.to_string()));
                return;
            }
        };
        // Held only for the request: an idle client keeps nothing alive.
        let Some(served) = served.upgrade() else {
            return;
        };
        let response = served.serve(request);
        drop(served);
        if let Err(e) = write_message(&mut stream, &response) {
            tracing::debug!(error = %e, "control client left");
            return;
        }
    }
}

impl Served {
    /// Run `request` on the supervisor.
    fn serve(&self, request: ControlRequest) -> ControlResponse {
        let (supervisor, platform) = (&self.supervisor, &*self.platform);
        let done = |r: Result<()>| {
            r.map_or_else(
                |e| ControlResponse::Failed(e.to_string()),
                |()| ControlResponse::Done,
            )
        };
        match request {
            ControlRequest::Status => ControlResponse::Status(supervisor.status()),
            ControlRequest::Ping(d) => match supervisor.ping(&d) {
                Ok(rtt) => {
                    ControlResponse::Pong(u64::try_from(rtt.as_millis()).unwrap_or(u64::MAX))
                }
                Err(e) => ControlResponse::Failed(e.to_string()),
            },
            ControlRequest::SendClipboard(d) => done(supervisor.send_clipboard(&d)),
            ControlRequest::SendFile(d, path) => {
                done(regular_file(platform, &path).and_then(|()| supervisor.send_file(&d, &path)))
            }
            ControlRequest::Handoff(d, item) => {
                let checked = match &item {
                    HandoffItem::File(path) => regular_file(platform, path),
                    HandoffItem::Url(_) => Ok(()),
                };
                done(checked.and_then(|()| supervisor.handoff(&d, item)))
            }
            ControlRequest::Ring(d) => done(supervisor.ring(&d)),
            ControlRequest::SetFeature(d, f, s) => done(supervisor.set_feature(&d, f, s)),
        }
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

/// A connection to the supervisor another process runs.
#[derive(Debug)]
pub struct ControlClient {
    stream: Mutex<LocalStream>,
}

impl ControlClient {
    /// Connect to `<state>/link.sock`, waiting a little for a supervisor
    /// that is still starting.
    fn connect(platform: &dyn Platform) -> Result<Self> {
        let socket = platform.state_dir()?.join(SOCKET_FILE);
        let start = Instant::now();
        loop {
            match platform.connect_local_socket(&socket) {
                Ok(stream) => {
                    return Ok(Self {
                        stream: Mutex::new(stream),
                    });
                }
                Err(_) if start.elapsed() < CONNECT_WAIT => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Send `request` and wait for the answer (requests from several
    /// threads take turns).
    ///
    /// # Errors
    ///
    /// The control socket failed ([`ControlError`] or IO).
    pub fn request(&self, request: &ControlRequest) -> Result<ControlResponse> {
        let mut stream = self.stream.lock().unwrap_or_else(PoisonError::into_inner);
        write_message(&mut *stream, request)?;
        read_message(&mut *stream)?.ok_or_else(|| ControlError::Closed.into())
    }
}

fn write_message(w: &mut impl Write, message: &impl Serialize) -> Result<()> {
    let mut body = Vec::new();
    ciborium::into_writer(message, &mut body).map_err(ControlError::from)?;
    let len = u32::try_from(body.len())
        .ok()
        .filter(|&n| n <= MAX_CONTROL_MESSAGE)
        .ok_or(ControlError::TooLarge)?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(&body)?;
    Ok(w.flush()?)
}

/// The next message; `None` when the other side hung up between messages.
fn read_message<T: DeserializeOwned>(r: &mut impl Read) -> Result<Option<T>> {
    let mut len = [0; 4];
    match r.read_exact(&mut len) {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        read => read?,
    }
    let len = u32::from_be_bytes(len);
    if len > MAX_CONTROL_MESSAGE {
        return Err(ControlError::TooLarge.into());
    }
    let mut body = vec![0; len as usize];
    r.read_exact(&mut body)?;
    Ok(Some(
        ciborium::from_reader(body.as_slice()).map_err(ControlError::from)?,
    ))
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) -> Result<JoinHandle<()>> {
    Ok(thread::Builder::new().name(name.to_owned()).spawn(f)?)
}
