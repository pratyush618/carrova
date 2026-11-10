//! Ctrl-C: the first press cancels the running session call, the second
//! quits at once.
//!
//! Commands [`arm`] the session's [`Cancel`] around each blocking engine
//! call that can be cancelled (scan, export, connect, send, transfer).
//! While armed, the first Ctrl-C triggers it and prints a notice; the
//! engine then stops promptly and leaves the session resumable (see
//! [`carrova_core::cancel`]). It never interrupts an apply: once apply
//! writes to the user's roots the engine ignores the cancel. Any further
//! press — or a press while nothing is armed (prompts, quick steps) —
//! quits immediately with exit code 130, as an unhandled SIGINT would.
//!
//! Signal delivery is OS-level, but this is a front-end, not the engine:
//! it uses the cross-platform `ctrlc` crate directly (no `cfg(target_os)`
//! here) rather than a `Platform` method. The handler runs on `ctrlc`'s own
//! thread, not in signal context, so it may lock and print.

use std::sync::{Mutex, MutexGuard, PoisonError};

use carrova_core::{Cancel, CoreError, ErrorKind};

use crate::output::OutputFormat;

/// Printed (human output) on the press that cancels.
const CANCELLING: &str = "Cancelling… (press Ctrl-C again to force quit)";
/// Exit code of a forced quit: 128 + SIGINT, like the default handler.
const FORCE_QUIT_EXIT: i32 = 130;

/// The process's one Ctrl-C state (signals are process-wide).
static PRESSES: Mutex<Presses> = Mutex::new(Presses::new());

/// Route Ctrl-C through [`arm`]ed cancels for the rest of the process.
/// Without a handler (installation failed), Ctrl-C keeps the OS default:
/// it quits at once.
pub fn install() {
    if let Err(e) = ctrlc::set_handler(on_ctrl_c) {
        tracing::warn!(error = %e, "Ctrl-C handler unavailable; Ctrl-C quits immediately");
    }
}

/// Let Ctrl-C cancel `cancel` until the returned guard is dropped. Arm one
/// call at a time (the guard disarms whatever is armed).
#[must_use = "Ctrl-C is armed only while the guard lives"]
pub fn arm(cancel: &Cancel, format: OutputFormat) -> Armed {
    presses().arm(cancel.clone(), format == OutputFormat::Human);
    Armed(())
}

/// Ctrl-C cancels the armed call while this lives ([`arm`]).
#[derive(Debug)]
pub struct Armed(());

impl Drop for Armed {
    fn drop(&mut self) {
        presses().disarm();
    }
}

/// `error` is the user cancelling (e.g. with Ctrl-C), not a failure worth
/// an error report.
#[must_use]
pub fn cancelled_by_user(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<CoreError>()
        .is_some_and(|e| e.class().kind == ErrorKind::Cancelled)
}

fn presses() -> MutexGuard<'static, Presses> {
    PRESSES.lock().unwrap_or_else(PoisonError::into_inner)
}

fn on_ctrl_c() {
    // Decide under the lock, act after releasing it.
    let press = presses().press();
    match press {
        Press::Cancelling { announce: true } => eprintln!("\n{CANCELLING}"),
        Press::Cancelling { announce: false } => {}
        Press::ForceQuit => std::process::exit(FORCE_QUIT_EXIT),
    }
}

/// What one Ctrl-C press does.
#[derive(Debug, PartialEq, Eq)]
enum Press {
    /// The armed call was cancelled; `announce`: print [`CANCELLING`].
    Cancelling { announce: bool },
    /// Quit now.
    ForceQuit,
}

/// The armed call (if any) and whether Ctrl-C was pressed before.
#[derive(Debug)]
struct Presses {
    armed: Option<Target>,
    pressed: bool,
}

/// A call Ctrl-C may cancel.
#[derive(Debug)]
struct Target {
    cancel: Cancel,
    /// Print the notice (human output; JSON output keeps stderr parseable).
    announce: bool,
}

impl Presses {
    const fn new() -> Self {
        Self {
            armed: None,
            pressed: false,
        }
    }

    fn arm(&mut self, cancel: Cancel, announce: bool) {
        self.armed = Some(Target { cancel, announce });
    }

    fn disarm(&mut self) {
        self.armed = None;
    }

    /// The first press cancels the armed call; any other press quits.
    fn press(&mut self) -> Press {
        let first = !std::mem::replace(&mut self.pressed, true);
        match &self.armed {
            Some(call) if first => {
                call.cancel.cancel();
                Press::Cancelling {
                    announce: call.announce,
                }
            }
            _ => Press::ForceQuit,
        }
    }
}

#[cfg(test)]
#[path = "../tests/interrupt/unit.rs"]
mod tests;
