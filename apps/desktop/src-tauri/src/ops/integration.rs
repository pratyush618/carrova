//! "Send with Carrova" file-manager entries: the platform writes them,
//! each launching this app ([`Platform::launcher`]) with `--send`.

use std::sync::Arc;

use carrova_platform::{FileManagerStatus, Platform, PlatformError};

use crate::command_error::CommandError;
use crate::state::AppState;
use crate::worker::blocking;

/// Whether this user's file-manager entries are written.
///
/// # Errors
///
/// The entries cannot be read.
pub async fn file_manager_status(state: &AppState) -> Result<FileManagerStatus, CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || Ok(platform.file_manager_status()?)).await
}

/// Write (`on`) or remove the file-manager entries; replies with the
/// result, file managers left untouched included.
///
/// # Errors
///
/// Unsupported here, or the entries cannot be written or removed.
pub async fn set_file_manager_entries(
    state: &AppState,
    on: bool,
) -> Result<FileManagerStatus, CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || {
        Ok(if on {
            install(platform.as_ref())?
        } else {
            platform.remove_file_manager_entries()?
        })
    })
    .await
}

/// Rewrite installed entries for this app's current launcher (it moves
/// on updates, e.g. a new AppImage). Blocking; failures are only logged,
/// so start-up never waits on or fails because of it.
pub fn refresh_file_manager_entries(platform: &dyn Platform) {
    let refreshed = platform
        .file_manager_status()
        .and_then(|status| status.installed.then(|| install(platform)).transpose());
    match refreshed {
        Ok(Some(status)) => tracing::info!(
            skipped = status.skipped.len(),
            "file-manager entries refreshed"
        ),
        Ok(None) => {}
        Err(error) => tracing::warn!(%error, "could not refresh the file-manager entries"),
    }
}

/// Write the entries for this app's launcher.
fn install(platform: &dyn Platform) -> Result<FileManagerStatus, PlatformError> {
    platform.install_file_manager_entries(&platform.launcher()?)
}
