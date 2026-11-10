//! Session model: identity, state machine, typed events.
//!
//! A session is one export (sender) or one import (receiver). States:
//!
//! ```text
//! Scanned → Selected → Planned → Transferring → Staged → Applying → Done
//!                                                          ↘ RolledBack (auto, on failure)
//! Done → RolledBack (manual `carrova rollback`)
//! any non-terminal → Failed
//! Transferring/Staged → Planned (resume: restage from scratch)
//! ```
//! Export sessions stop after `Transferring → Done` (archive written).
//! Front-ends render [`Event`]s; they never poke engine state.

mod events;
pub mod log;
pub mod progress;
mod state;

pub use events::{Event, EventSink, NullSink, PauseReason, RollbackOutcome, VecSink};
pub use log::LogSink;
pub use progress::{Metered, Phase, ProgressMeter};
pub use state::{Role, SessionId, State};
