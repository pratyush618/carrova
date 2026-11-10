//! Session workers: one `std::thread` per live session that owns the
//! engine's session object and runs requests against it, one at a time.
//!
//! Engine calls block (file I/O, Argon2, and the transport's private
//! tokio runtime via `block_on`), so they must never run on Tauri's async
//! runtime. Async commands send a job and await its oneshot reply; the
//! session's `Cancel` lives outside the worker, so a cancel works while a
//! job runs.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;

use futures_channel::oneshot;

use crate::command_error::CommandError;

/// A request for the worker: runs with exclusive access to the object.
type Job<T> = Box<dyn FnOnce(&mut T) + Send>;

/// Handle to a worker thread owning a `T`. Cheap to clone; the thread
/// ends once every handle is dropped and its last job returned.
#[derive(Debug)]
pub struct Worker<T> {
    jobs: mpsc::Sender<Job<T>>,
    busy: Arc<AtomicBool>,
}

impl<T> Clone for Worker<T> {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
            busy: Arc::clone(&self.busy),
        }
    }
}

impl<T: 'static> Worker<T> {
    /// Start a worker thread named `name` that first builds its object
    /// with `open` (on the worker thread: opening blocks too). Resolves
    /// with the handle and `open`'s extra result, or `open`'s error (the
    /// thread then ends).
    pub async fn open<R, E>(
        name: &str,
        open: impl FnOnce() -> Result<(T, R), E> + Send + 'static,
    ) -> Result<(Self, R), CommandError>
    where
        R: Send + 'static,
        E: Into<CommandError> + Send + 'static,
    {
        let (jobs, queue) = mpsc::channel::<Job<T>>();
        let (opened, reply) = oneshot::channel();
        let busy = Arc::new(AtomicBool::new(true));
        let flag = Arc::clone(&busy);
        thread::Builder::new()
            .name(format!("carrova-{name}"))
            .spawn(move || {
                let mut object = match open() {
                    Ok((object, extra)) => {
                        // A dropped receiver only means nobody waits.
                        let _ = opened.send(Ok(extra));
                        object
                    }
                    Err(error) => {
                        let _ = opened.send(Err(error.into()));
                        return;
                    }
                };
                flag.store(false, Ordering::Release);
                for job in queue {
                    let _busy = Busy::mark(&flag);
                    job(&mut object);
                }
            })?;
        let extra = reply.await.map_err(|_| CommandError::worker_gone())??;
        Ok((Self { jobs, busy }, extra))
    }

    /// Run `job` on the worker and await its result. Jobs run in order.
    pub async fn call<R: Send + 'static>(
        &self,
        job: impl FnOnce(&mut T) -> R + Send + 'static,
    ) -> Result<R, CommandError> {
        let (done, reply) = oneshot::channel();
        self.jobs
            .send(Box::new(move |object: &mut T| {
                let _ = done.send(job(object));
            }))
            .map_err(|_| CommandError::worker_gone())?;
        reply.await.map_err(|_| CommandError::worker_gone())
    }

    /// Whether a job (or the opening) is running right now.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }
}

/// Marks a worker busy while alive; clears the mark even if the job
/// panics.
struct Busy<'a>(&'a AtomicBool);

impl<'a> Busy<'a> {
    fn mark(flag: &'a AtomicBool) -> Self {
        flag.store(true, Ordering::Release);
        Self(flag)
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Run a blocking, session-less engine call (history, rollback, recipes,
/// settings) off the async runtime.
pub async fn blocking<R: Send + 'static>(
    call: impl FnOnce() -> Result<R, CommandError> + Send + 'static,
) -> Result<R, CommandError> {
    tauri::async_runtime::spawn_blocking(call)
        .await
        .map_err(|_| CommandError::worker_gone())?
}

#[cfg(test)]
#[path = "../tests/worker/unit.rs"]
mod tests;
