//! The transport's private tokio runtime.
//!
//! One multi-thread runtime per process with a few workers for network
//! I/O. The public API is blocking: it drives futures with
//! [`Runtime::block_on`] from the caller's thread, so it must never be
//! called from inside this runtime (the engine is synchronous).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use tokio::runtime::{Builder, Runtime};
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::cancel::Watch;
use crate::error::{Result, SetupFailure, TransportError};
use crate::tuning::{CLOSE_LINGER, RUNTIME_WORKER_THREADS};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// The shared runtime, built on first use. Lives for the whole process, so
/// QUIC drivers keep flushing (e.g. close frames) after handles are gone.
pub(crate) fn runtime() -> Result<&'static Runtime> {
    if let Some(rt) = RUNTIME.get() {
        return Ok(rt);
    }
    let rt = Builder::new_multi_thread()
        .worker_threads(RUNTIME_WORKER_THREADS)
        .thread_name("carrova-net")
        .enable_io()
        .enable_time()
        .build()
        .map_err(TransportError::Io)?;
    // A concurrent first call may have won the race; its runtime is used
    // and ours is dropped here, on a caller thread (never inside a runtime).
    Ok(RUNTIME.get_or_init(|| rt))
}

/// Something a background task watches to stop early.
pub(crate) trait Stoppable: Send + Sync {
    /// Ask every task to wind down (resetting its streams).
    fn stop(&self);
}

/// A one-way stop signal shared by a driver task and its stream tasks.
#[derive(Debug, Default)]
pub(crate) struct Stop {
    stopped: AtomicBool,
    notify: Notify,
}

impl Stop {
    /// Resolves once [`Stoppable::stop`] was called.
    pub(crate) async fn stopped(&self) {
        loop {
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.stopped.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }
}

impl Stoppable for Stop {
    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }
}

/// A spawned driver task tied to the calling thread's scope.
///
/// Consumed normally with [`Driver::join`]. If the caller unwinds instead
/// (e.g. a panicking callback), dropping the guard stops the task and waits
/// for it, so every stream it owns is reset before the connection can be
/// closed (quinn will not send a close frame while stream data is pending).
pub(crate) struct Driver<T> {
    handle: Option<JoinHandle<Result<T>>>,
    stop: Arc<dyn Stoppable>,
}

impl<T: Send + 'static> Driver<T> {
    /// Spawn `task`, stopped through `stop` when the caller unwinds or
    /// `cancel` fires. A cancelled task gets [`CLOSE_LINGER`] to wind down
    /// (reset its streams, tell the peer), then the driver ends with
    /// [`TransportError::Cancelled`].
    pub(crate) fn spawn(
        task: impl Future<Output = Result<T>> + Send + 'static,
        stop: Arc<dyn Stoppable>,
        cancel: Watch,
    ) -> Result<Self> {
        let stopper = Arc::clone(&stop);
        let task = async move {
            tokio::pin!(task);
            tokio::select! {
                biased;
                result = &mut task => result,
                () = cancel.cancelled() => {
                    stopper.stop();
                    let _ = timeout(CLOSE_LINGER, task).await;
                    Err(TransportError::Cancelled)
                }
            }
        };
        Ok(Self {
            handle: Some(runtime()?.spawn(task)),
            stop,
        })
    }

    /// Wait for the task's result.
    pub(crate) fn join(mut self) -> Result<T> {
        let handle = self
            .handle
            .take()
            .ok_or_else(|| TransportError::from(SetupFailure::DriverJoined))?;
        runtime()?
            .block_on(handle)
            .map_err(|e| TransportError::internal("driver task", e))?
    }
}

impl<T> Drop for Driver<T> {
    fn drop(&mut self) {
        if let (Some(handle), Some(rt)) = (self.handle.take(), RUNTIME.get()) {
            self.stop.stop();
            let _ = rt.block_on(handle);
        }
    }
}

impl<T> std::fmt::Debug for Driver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Driver")
            .field("running", &self.handle.is_some())
            .finish_non_exhaustive()
    }
}
