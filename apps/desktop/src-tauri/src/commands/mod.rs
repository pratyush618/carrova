//! Tauri commands: one per `CarrovaPort` method (snake_case of its name),
//! each forwarding to its operation in [`crate::ops`]. No logic lives
//! here. Arguments arrive by name (camelCase in JS, snake_case here).
//!
//! Commands that reach the engine pass their result through `reply`,
//! the one place a failed command becomes an error report.

pub mod integration;
pub mod receive;
pub mod recipes;
pub mod reports;
pub mod send;
pub mod sessions;
pub mod settings;

use tauri::{AppHandle, Runtime};

use crate::command_error::CommandError;
use crate::error_report::record_failure;

/// A command's `result` on its way to the UI: an engine failure is
/// recorded as an error report first (only with the user's consent,
/// checked by the report store).
fn reply<T, R: Runtime>(
    app: &AppHandle<R>,
    result: Result<T, CommandError>,
) -> Result<T, CommandError> {
    if let Err(error) = &result
        && error.is_engine_failure()
    {
        record_failure(app, error);
    }
    result
}
