//! "Send with Carrova" requests from outside the app: `--send PATH…` on
//! the command line (a file-manager entry, or a second launch forwarded
//! by the single-instance plugin). The macOS Quick Action starts a fresh
//! instance with `open -n … --args --send`, so it arrives the same way.
//!
//! Plumbing only: paths are queued as text for the UI, which decides what
//! to do with them (and whether it is busy); the engine judges each path
//! when the UI picks it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use carrova_view::ipc::BridgeEvent;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// The flag file-manager entries launch the app with, before the paths.
pub const SEND_FLAG: &str = "--send";

/// Paths sent from outside, waiting for the UI to take them.
#[derive(Debug, Clone, Default)]
pub struct SendRequests(Arc<Mutex<Vec<String>>>);

impl SendRequests {
    /// Every queued path, oldest first, emptying the queue.
    #[must_use]
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.lock())
    }

    /// Queue `paths`, as text: a name that is not valid Unicode keeps its
    /// place (lossily) and the engine refuses it when picked.
    fn push(&self, paths: &[PathBuf]) {
        let text = paths.iter().map(|p| p.to_string_lossy().into_owned());
        self.lock().extend(text);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<String>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The paths of a `--send` command line: everything after the flag,
/// relative ones joined to `cwd` (the launching file manager's folder).
/// `None` for a normal start (no flag, or no path after it).
#[must_use]
pub fn send_paths(argv: &[OsString], cwd: &Path) -> Option<Vec<PathBuf>> {
    let flag = argv.iter().position(|arg| arg == SEND_FLAG)?;
    let paths: Vec<_> = argv[flag + 1..].iter().map(|arg| cwd.join(arg)).collect();
    (!paths.is_empty()).then_some(paths)
}

/// Queue `paths` and tell the UI ([`BridgeEvent::SendRequested`]); it
/// fetches them with `take_send_request`.
pub fn forward(state: &AppState, paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }
    tracing::info!(paths = paths.len(), "send requested from outside");
    state.pending_send.push(paths);
    state.out.emit(&BridgeEvent::SendRequested);
}

/// [`forward`] once the app `state` exists; before that, only queue
/// `paths` in `queue` (the one the state is built with), which the UI
/// drains when it starts.
fn deliver(state: Option<&AppState>, queue: &SendRequests, paths: &[PathBuf]) {
    match state {
        Some(state) => forward(state, paths),
        None => queue.push(paths),
    }
}

/// A second launch (argv and working directory, as text) reached the
/// running app: queue its `--send` paths, if any, in `queue` (shared with
/// [`AppState::pending_send`], so none are lost while the app starts) and
/// open the window.
pub fn on_second_instance(app: &AppHandle, queue: &SendRequests, argv: Vec<String>, cwd: &str) {
    let argv: Vec<OsString> = argv.into_iter().map(OsString::from).collect();
    let paths = send_paths(&argv, Path::new(cwd)).unwrap_or_default();
    deliver(app.try_state::<AppState>().as_deref(), queue, &paths);
    crate::tray::on_main(app, |app| {
        crate::tray::open_main(app);
    });
}

#[cfg(test)]
#[path = "../tests/send_request/unit.rs"]
mod tests;
