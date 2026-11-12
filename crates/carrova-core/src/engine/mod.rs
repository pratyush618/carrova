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

use carrova_crypto::identity::DeviceKey;
use carrova_platform::Platform;
use carrova_plugin::Plugin;
use carrova_recipe::Recipe;

use crate::devices::KnownDevices;
use crate::session::{Event, EventSink};

mod archive;
mod checklist;
mod export;
mod import;
mod receive;
mod redact;
mod remember;
mod resume;
mod send;
mod session;
pub mod sessions;
mod transfer;

pub use export::Exporter;
pub use import::{Importer, PlanApproval};
pub use send::{CodeOffer, SendHandle, SendOutcome, SendPrompt, SendReport};

/// Shared engine context: the platform, the loaded recipes and plugins,
/// and the front-end's event sink. Cheap to clone.
#[derive(Clone)]
pub struct Engine {
    platform: Arc<dyn Platform>,
    recipes: Arc<[Recipe]>,
    plugins: Arc<[Plugin]>,
    sink: Arc<dyn EventSink>,
    /// Remember devices moved with using a code (default on).
    remember_devices: bool,
    /// Whether to store a remembered device's hardware address as a wake
    /// hint, asked as each move is remembered (default off).
    wol_hints: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl Engine {
    /// An engine on `platform` with `recipes` and no plugins, reporting
    /// to `sink`.
    pub fn new(
        platform: Arc<dyn Platform>,
        recipes: impl Into<Arc<[Recipe]>>,
        sink: Arc<dyn EventSink>,
    ) -> Self {
        Self {
            platform,
            recipes: recipes.into(),
            plugins: Arc::new([]),
            sink,
            remember_devices: true,
            wol_hints: Arc::new(|| false),
        }
    }

    /// This engine remembering devices after code-paired moves (`true`,
    /// the default) or not: then no device identity is made or proven,
    /// and nothing is stored.
    #[must_use]
    pub fn with_remember_devices(mut self, remember: bool) -> Self {
        self.remember_devices = remember;
        self
    }

    /// This engine saving a hardware-address hint (for Wake-on-LAN) with
    /// each device it remembers while `on` says so (default: never). `on`
    /// is asked under the devices lock as each move is remembered, so a
    /// setting turned off mid-move holds once it is saved; off makes the
    /// move drop the device's saved hint (see
    /// [`KnownDevices::clear_wake_hints`] to drop them at once).
    #[must_use]
    pub fn with_wol_hints(mut self, on: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        self.wol_hints = Arc::new(on);
        self
    }

    /// This install's device key when it remembers devices (made on first
    /// need), for the transport to prove at code pairing. An unusable key
    /// never blocks a move: it pairs without identity (nothing is
    /// remembered) and the user is warned.
    fn device_key(&self) -> Option<Arc<DeviceKey>> {
        if !self.remember_devices {
            return None;
        }
        let platform = &*self.platform;
        match KnownDevices::open(platform).and_then(|known| known.device_key(platform)) {
            Ok(key) => Some(Arc::new(key)),
            Err(e) => {
                let kind = e.class().kind;
                tracing::warn!(?kind, "device key unusable; pairing without identity");
                self.sink.emit(Event::Warning {
                    message: format!("devices are not remembered: {e}"),
                });
                None
            }
        }
    }

    /// This engine with `plugins` (loaded from the same source as the
    /// recipes): items whose local recipe names one pass through it.
    #[must_use]
    pub fn with_plugins(mut self, plugins: impl Into<Arc<[Plugin]>>) -> Self {
        self.plugins = plugins.into();
        self
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

    /// The loaded plugins.
    #[must_use]
    pub fn plugins(&self) -> &[Plugin] {
        &self.plugins
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
            .field("plugins", &self.plugins.len())
            .field("remember_devices", &self.remember_devices)
            .finish_non_exhaustive()
    }
}
