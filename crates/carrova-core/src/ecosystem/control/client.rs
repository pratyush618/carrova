//! A process's connection to the supervisor another process runs.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use carrova_link::ReceivedClip;
use carrova_platform::{LocalStream, Platform};

use super::server::SUBSCRIBER_QUEUE;
use super::wire::{
    ControlError, ControlRequest, ControlResponse, MAX_CONTROL_MESSAGE, MAX_CONTROL_REPLY,
    read_message, write_message,
};
use super::{SOCKET_FILE, spawn};
use crate::error::Result;
use crate::session::Event;

/// How long a client waits for a just-started supervisor to listen.
pub(super) const CONNECT_WAIT: Duration = Duration::from_secs(5);
/// How often it tries meanwhile.
pub(super) const CONNECT_RETRY: Duration = Duration::from_millis(50);

/// A connection to the supervisor another process runs.
#[derive(Debug)]
pub struct ControlClient {
    platform: Arc<dyn Platform>,
    socket: PathBuf,
    stream: Mutex<LocalStream>,
}

impl ControlClient {
    /// Connect to `<state>/link.sock` (one try).
    pub(super) fn connect(platform: &Arc<dyn Platform>) -> Result<Self> {
        let socket = platform.state_dir()?.join(SOCKET_FILE);
        let stream = Mutex::new(platform.connect_local_socket(&socket)?);
        Ok(Self {
            platform: Arc::clone(platform),
            socket,
            stream,
        })
    }

    /// Send `request` and wait for the answer (short requests from several
    /// threads take turns; a send, which waits for the peer's answer and
    /// the transfer, runs on a connection of its own so it holds up none
    /// of them). A refusal is [`ControlError::Failed`].
    pub(super) fn request(&self, request: &ControlRequest) -> Result<ControlResponse> {
        if matches!(
            request,
            ControlRequest::SendFile(..)
                | ControlRequest::Handoff(..)
                | ControlRequest::RetryFile(_)
        ) {
            return exchange(&mut dial(&*self.platform, &self.socket)?, request);
        }
        let mut stream = self.stream.lock().unwrap_or_else(PoisonError::into_inner);
        exchange(&mut stream, request)
    }

    /// The clip history: a count, then one message per clip.
    pub(super) fn clip_history(&self) -> Result<Vec<ReceivedClip>> {
        let mut stream = self.stream.lock().unwrap_or_else(PoisonError::into_inner);
        let ControlResponse::ClipHistory(count) =
            exchange(&mut stream, &ControlRequest::ClipHistory)?
        else {
            return Err(ControlError::Unexpected.into());
        };
        (0..count)
            .map(|_| {
                read_message(&mut *stream, MAX_CONTROL_REPLY)?
                    .ok_or_else(|| ControlError::Closed.into())
            })
            .collect()
    }

    /// The supervisor's events, read on a thread of their own connection
    /// (it ends when the supervisor stops, or at the next event once the
    /// receiver is dropped).
    pub(super) fn subscribe(&self) -> Result<Receiver<Event>> {
        let mut stream = dial(&*self.platform, &self.socket)?;
        match exchange(&mut stream, &ControlRequest::Subscribe)? {
            ControlResponse::Done => {}
            _ => return Err(ControlError::Unexpected.into()),
        }
        // Bounded: a reader that stalls stops this thread reading, and the
        // server then drops the subscription.
        let (tx, rx) = mpsc::sync_channel(SUBSCRIBER_QUEUE);
        spawn("carrova-control-events", move || {
            while let Ok(Some(event)) = read_message(&mut stream, MAX_CONTROL_REPLY) {
                if tx.send(event).is_err() {
                    return;
                }
            }
        })?;
        Ok(rx)
    }
}

/// Connect to `socket`, waiting a little for a supervisor that is still
/// starting.
fn dial(platform: &dyn Platform, socket: &Path) -> Result<LocalStream> {
    let start = Instant::now();
    loop {
        match platform.connect_local_socket(socket) {
            Ok(stream) => return Ok(stream),
            Err(_) if start.elapsed() < CONNECT_WAIT => thread::sleep(CONNECT_RETRY),
            Err(e) => return Err(e.into()),
        }
    }
}

fn exchange(stream: &mut LocalStream, request: &ControlRequest) -> Result<ControlResponse> {
    write_message(stream, request, MAX_CONTROL_MESSAGE)?;
    match read_message(stream, MAX_CONTROL_REPLY)? {
        Some(ControlResponse::Failed(reason)) => Err(ControlError::Failed(reason).into()),
        Some(response) => Ok(response),
        None => Err(ControlError::Closed.into()),
    }
}
