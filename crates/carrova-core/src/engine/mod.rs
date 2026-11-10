//! Session engine: the only API front-ends call. Wires scan → select →
//! capture → archive or LAN send (export) and archive or LAN receive → plan
//! → approve → stage → apply (import), persisting state at every
//! transition and emitting typed [`crate::session::Event`]s. CLI and (Phase 4) UI behave
//! identically because both only drive these types.
//!
//! # Modules
//! - [`Engine`]: the shared context (platform, recipes, front-end sink).
//! - `session`: the persisted record, its log and the one transition helper.
//! - `export` / `send`: [`Exporter`] to an archive or a LAN receiver.
//! - `import` / `receive`: [`Importer`] from an archive or a LAN sender.
//! - `resume`: continuing an import after a restart.
//! - [`sessions`]: listing and maintenance without a live session.
//!
//! # State machine
//! Every state change goes through one helper: it checks
//! [`crate::session::State::can_transition`], persists `session.json` (atomic), and only
//! then emits [`crate::session::Event::StateChanged`]. A failure moves the session to
//! `Failed` with the error message recorded (messages never hold secrets
//! or file contents).
//!
//! # Logs
//! Each session appends every event to `log.jsonl` ([`crate::session::LogSink`]) and
//! forwards it to the front-end's sink. [`sessions::export_log`] writes a
//! redacted copy (see [`crate::session::log`]).
//!
//! # Resume
//! Archive export sessions are one-shot and are not resumable (re-run the
//! export). LAN sessions resume on both sides ([`Exporter::resume_send`],
//! [`Importer::resume_lan`]). [`Importer::resume_archive`] continues an
//! archive import after a restart, from its last persisted state:
//! - `Selected`: the archive is reopened; plan again.
//! - `Planned`, `Transferring`, `Staged`: any staging is wiped and the plan
//!   is recomputed from the persisted options against the *current*
//!   filesystem (state `Planned`). The earlier approval is kept only if the
//!   new plan has the same actions and handlers and no blockers; otherwise
//!   the user must approve again. `apply` then restages from scratch.
//! - `Applying` (crash mid-apply): the journal is replayed by
//!   `rollback::rollback` *before* the archive is opened (so recovery
//!   works even if the archive is gone), staging is wiped and the session
//!   ends `RolledBack` (or `Failed` if some step needs manual attention).
//!   The returned importer is finished; start a new import to retry.
//! - `Done`, `RolledBack`, `Failed`: nothing to resume (error).
//!
//! Every reopen of the archive (staging, resume) checks it against the
//! [`crate::store::ArchiveFingerprint`] taken when the import was opened; a
//! swapped archive fails the session ("archive changed since plan") before
//! anything is staged or written.
//!
//! There is no cross-process locking: one process drives a session at a time.
//!
//! # Threads and cancellation
//! Every session object is `Send`: a front-end drives it on a worker
//! thread and stops it from another through the object's
//! [`crate::Cancel`] (`canceller()`, taken before the blocking call). A
//! cancel never interrupts an apply; see [`crate::cancel`].

use std::fmt;
use std::sync::Arc;

use carrova_platform::Platform;
use carrova_recipe::Recipe;

use crate::session::EventSink;

mod archive;
mod checklist;
mod export;
mod import;
mod receive;
mod redact;
mod resume;
mod send;
mod session;
pub mod sessions;
mod transfer;

pub use export::Exporter;
pub use import::{Importer, PlanApproval};
pub use send::{CodeOffer, SendHandle, SendOutcome, SendPrompt, SendReport};

/// Shared engine context: the platform, the loaded recipes and the
/// front-end's event sink. Cheap to clone.
#[derive(Clone)]
pub struct Engine {
    platform: Arc<dyn Platform>,
    recipes: Arc<[Recipe]>,
    sink: Arc<dyn EventSink>,
}

impl Engine {
    /// An engine on `platform` with `recipes`, reporting to `sink`.
    pub fn new(
        platform: Arc<dyn Platform>,
        recipes: impl Into<Arc<[Recipe]>>,
        sink: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            platform,
            recipes: recipes.into(),
            sink,
        }
    }

    /// The platform every session runs on.
    #[must_use]
    pub fn platform(&self) -> &Arc<dyn Platform> {
        &self.platform
    }

    /// The loaded recipes.
    #[must_use]
    pub fn recipes(&self) -> &[Recipe] {
        &self.recipes
    }

    /// The front-end's event sink.
    #[must_use]
    pub fn sink(&self) -> &Arc<dyn EventSink> {
        &self.sink
    }
}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Engine")
            .field("platform", &self.platform)
            .field("recipes", &self.recipes.len())
            .finish_non_exhaustive()
    }
}
