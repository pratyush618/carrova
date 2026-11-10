//! Opt-in crash/error reports: the desktop wiring of `carrova-report`.
//!
//! All policy (redaction, consent gate, retention, issue text) lives in
//! `carrova-report`, shared with the CLI; this module only resolves where
//! the store lives, writes its files `0600` through the platform
//! (`PlatformReportFs`) and records failures the UI is shown
//! ([`record_failure`]). The IPC commands are in
//! [`crate::commands::reports`].

use std::error::Error;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

use carrova_platform::{Platform, PlatformError, SemanticRoot};
use carrova_report::{
    ErrorReport, ReportContext, ReportError, ReportFs, ReportId, ReportStore, install_panic_hook,
};
use tauri::{AppHandle, Manager, Runtime};

/// Carrova version recorded in every report.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Permission bits of every report and the consent file.
const PRIVATE_MODE: u32 = 0o600;

/// The report store plus the context every new report carries. Managed
/// as Tauri state.
#[derive(Debug)]
pub struct Reports {
    store: ReportStore,
    context: ReportContext,
}

impl Reports {
    /// Store under the platform's state directory, written through it;
    /// context from its OS name and the user's home (redaction of home
    /// paths and user name).
    ///
    /// # Errors
    ///
    /// [`PlatformError`] if the state directory cannot be resolved.
    pub fn from_platform(platform: Arc<dyn Platform>) -> Result<Self, PlatformError> {
        let state_dir = platform.state_dir()?;
        let home = platform.resolve_root(SemanticRoot::Home).ok();
        let context = ReportContext::new(APP_VERSION, platform.os().to_string(), home.as_deref());
        let store = ReportStore::in_state_dir(&state_dir, Arc::new(PlatformReportFs(platform)));
        Ok(Self { store, context })
    }

    /// The report store (for the report commands).
    #[must_use]
    pub fn store(&self) -> &ReportStore {
        &self.store
    }

    /// Installs the reporting panic hook. It writes only while the
    /// persisted consent is `Allowed` (checked at panic time).
    pub fn install_panic_hook(&self) {
        install_panic_hook(self.store.clone(), self.context.clone());
    }

    /// Records an error the user saw (no-op without consent).
    ///
    /// # Errors
    ///
    /// [`ReportError`] if the report cannot be written.
    pub fn record(&self, error: &(dyn Error + 'static)) -> Result<Option<ReportId>, ReportError> {
        self.store
            .save(&ErrorReport::from_error(error, &self.context))
    }

    /// [`Self::record`] for a failed command: a report that cannot be
    /// written is logged, never turned into a second error.
    pub fn record_failure(&self, error: &(dyn Error + 'static)) {
        match self.record(error) {
            Ok(Some(id)) => tracing::info!(%id, "error report saved"),
            Ok(None) => {}
            Err(e) => tracing::warn!(error = %e, "could not save the error report"),
        }
    }
}

/// Records a failure a command returns to the UI as an error report
/// (consent-gated in `ReportStore::save`, as the CLI does). A no-op if
/// the report state is not managed (no state directory).
pub fn record_failure<R: Runtime>(app: &AppHandle<R>, error: &(dyn Error + 'static)) {
    if let Some(reports) = app.try_state::<Reports>() {
        reports.record_failure(error);
    }
}

/// [`ReportFs`] over a [`Platform`]: owner-only files, never written
/// through a symlink.
#[derive(Debug)]
struct PlatformReportFs(Arc<dyn Platform>);

impl ReportFs for PlatformReportFs {
    fn create_private(&self, path: &Path) -> io::Result<File> {
        self.0
            .create_new_file(path, Some(PRIVATE_MODE))
            .map_err(into_io)
    }

    fn replace(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.0.rename_replace(from, to).map_err(into_io)
    }
}

/// The underlying I/O error (the store adds the path itself).
fn into_io(err: PlatformError) -> io::Error {
    match err {
        PlatformError::Io { source, .. } => source,
        other => io::Error::other(other),
    }
}

#[cfg(test)]
#[path = "../tests/error_report/unit.rs"]
mod tests;
