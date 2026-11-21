//! One run at a time of a long command (an app update install): a second
//! call while one runs is refused, so it can neither reset the first's
//! controls nor run the same steps twice.

use std::sync::atomic::{AtomicBool, Ordering};

use carrova_view::ipc::ErrorCode;

use crate::command_error::CommandError;

/// Whether the command runs now.
#[derive(Debug, Default)]
pub struct InFlight(AtomicBool);

/// The run in progress; it ends when this drops.
#[derive(Debug)]
pub struct Running<'a>(&'a AtomicBool);

impl InFlight {
    /// Start a run of `what`.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::Session`] while another run of it is in progress.
    pub fn start(&self, what: &str) -> Result<Running<'_>, CommandError> {
        if self.0.swap(true, Ordering::AcqRel) {
            return Err(CommandError::new(
                ErrorCode::Session,
                format!("{what} is already running"),
            ));
        }
        Ok(Running(&self.0))
    }

    /// Whether a run is in progress.
    pub fn running(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "../tests/in_flight/unit.rs"]
mod tests;
