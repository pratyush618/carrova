//! A process's lasting hold on the user's supervisor: when the process
//! hosting it exits, this one attaches again (and usually becomes the host,
//! the exited one's lock being free).

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use super::{ControlError, SupervisorHandle, attach_or_start_with};
use crate::ecosystem::{PeerSource, PresenceSource, SupervisorConfig};
use crate::engine::Engine;
use crate::error::{CoreError, Result};
use crate::session::{Event, EventSink};

/// First wait after a failed subscription in [`SharedSupervisor::forward`].
const RETRY_MIN: Duration = Duration::from_secs(1);
/// Longest such wait (it doubles up to it).
const RETRY_MAX: Duration = Duration::from_secs(5);

/// The user's supervisor, held for the life of a process (an app, a
/// phone's service): every call goes to the current [`SupervisorHandle`],
/// and one that failed because its host exited attaches again first.
pub struct SharedSupervisor {
    engine: Engine,
    source: Arc<dyn PeerSource>,
    config: SupervisorConfig,
    current: Mutex<Arc<SupervisorHandle>>,
}

impl std::fmt::Debug for SharedSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedSupervisor")
            .field("current", &self.current)
            .finish_non_exhaustive()
    }
}

impl SharedSupervisor {
    /// [`super::attach_or_start`], held.
    ///
    /// # Errors
    ///
    /// As [`super::attach_or_start`].
    pub fn attach(engine: Engine) -> Result<Self> {
        let source = Arc::new(PresenceSource::new(Arc::clone(engine.platform())));
        Self::attach_with(engine, source, SupervisorConfig::default())
    }

    /// [`attach_or_start_with`], held: attaching again uses the same
    /// `engine`, `source` and `config`.
    ///
    /// # Errors
    ///
    /// As [`attach_or_start_with`].
    pub fn attach_with(
        engine: Engine,
        source: Arc<dyn PeerSource>,
        config: SupervisorConfig,
    ) -> Result<Self> {
        let handle = attach_or_start_with(engine.clone(), Arc::clone(&source), config)?;
        Ok(Self {
            engine,
            source,
            config,
            current: Mutex::new(Arc::new(handle)),
        })
    }

    /// Run `f` on the supervisor. If it failed because the process hosting
    /// it is gone, attach again and run it once more.
    ///
    /// # Errors
    ///
    /// `f`'s error, or attaching again failed.
    pub fn call<R>(&self, f: impl Fn(&SupervisorHandle) -> Result<R>) -> Result<R> {
        let handle = Arc::clone(&self.lock());
        match f(&handle) {
            Err(error) if host_gone(&handle, &error) => {
                tracing::debug!("the supervisor's host is gone; attaching again");
                let again = self.reattach(&handle)?;
                f(&again)
            }
            done => done,
        }
    }

    /// Hand every supervisor event to `sink`, for as long as the process
    /// runs. A stream that ends (a subscriber that fell behind is dropped,
    /// a host exited) is subscribed again, attaching again as needed;
    /// failed tries wait 1 s, doubling up to 5 s.
    pub fn forward(&self, sink: &dyn EventSink) -> ! {
        let mut wait = RETRY_MIN;
        loop {
            match self.call(SupervisorHandle::subscribe) {
                Ok(events) => {
                    wait = RETRY_MIN;
                    drain(events, sink);
                    tracing::debug!("supervisor events ended; subscribing again");
                }
                Err(error) => {
                    tracing::warn!(%error, "no supervisor events");
                    thread::sleep(wait);
                    wait = (wait * 2).min(RETRY_MAX);
                }
            }
        }
    }

    /// Replace `failed` (unless another call already did) and return the
    /// handle now current.
    fn reattach(&self, failed: &Arc<SupervisorHandle>) -> Result<Arc<SupervisorHandle>> {
        let mut current = self.lock();
        if Arc::ptr_eq(&current, failed) {
            let handle =
                attach_or_start_with(self.engine.clone(), Arc::clone(&self.source), self.config)?;
            *current = Arc::new(handle);
        }
        Ok(Arc::clone(&current))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Arc<SupervisorHandle>> {
        self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn drain(events: Receiver<Event>, sink: &dyn EventSink) {
    events.into_iter().for_each(|event| sink.emit(event));
}

/// Whether `error` from `handle` means its host is gone: a client's
/// failure other than the host's own refusal.
fn host_gone(handle: &SupervisorHandle, error: &CoreError) -> bool {
    matches!(handle, SupervisorHandle::Remote(_))
        && !matches!(
            error,
            CoreError::Control(ControlError::Failed(_) | ControlError::NotAFile)
        )
}
