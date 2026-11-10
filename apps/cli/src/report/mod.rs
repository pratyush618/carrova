//! Opt-in local error reports: the panic hook, recording a failed
//! command, and `carrova report`.
//!
//! All policy (redaction, the consent gate in `ReportStore::save`,
//! retention, issue text) lives in `carrova-report`, shared with the
//! desktop app; this module only wires it to the current platform (files
//! written `0600` through [`fs::PlatformReportFs`]) and the command line.

mod args;
mod command;
mod fs;

use std::sync::Arc;

use carrova_platform::{Platform, PlatformError, SemanticRoot};
use carrova_report::{ErrorReport, ReportContext, ReportStore, install_panic_hook};
use fs::PlatformReportFs;

pub use args::ReportArgs;
pub use command::run;

/// The report store plus the context every new report carries.
#[derive(Debug)]
pub struct Reports {
    store: ReportStore,
    context: ReportContext,
}

impl Reports {
    /// Store under `platform`'s state directory, written through it;
    /// context from its OS name and the user's home (redaction of home
    /// paths and the user name).
    pub fn from_platform(platform: Arc<dyn Platform>) -> Result<Self, PlatformError> {
        let state_dir = platform.state_dir()?;
        let home = platform.resolve_root(SemanticRoot::Home).ok();
        let context = ReportContext::new(
            env!("CARGO_PKG_VERSION"),
            platform.os().to_string(),
            home.as_deref(),
        );
        let store = ReportStore::in_state_dir(&state_dir, Arc::new(PlatformReportFs(platform)));
        Ok(Self { store, context })
    }

    /// Installs the reporting panic hook (writes only while consent is
    /// `Allowed`, checked when a panic happens).
    pub fn install_panic_hook(&self) {
        install_panic_hook(self.store.clone(), self.context.clone());
    }

    /// Records the error a command failed with (a no-op without consent)
    /// and tells the user where it went. Never fails the exit itself.
    pub fn record(&self, error: &anyhow::Error) {
        match self
            .store
            .save(&ErrorReport::from_error(error.as_ref(), &self.context))
        {
            Ok(Some(id)) => eprintln!(
                "A redacted error report was saved on this computer: `carrova report show {id}`"
            ),
            Ok(None) => {}
            Err(e) => eprintln!("carrova: could not save the error report: {e}"),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/report/unit.rs"]
mod tests;
