//! Carrova session engine.
//!
//! Phase 1: scan orchestration ([`catalog`], [`scan`](mod@scan)).
//! Phase 2: archive migration sessions — [`engine::Exporter`] /
//! [`engine::Importer`] drive [`select`] → [`capture`] → [`export`] and
//! [`plan`] → stage → apply / rollback, persisted by [`store`],
//! reporting through typed [`session::Event`]s. Front-ends only render.
//! Known devices ([`devices`]): this install's identity key and the
//! devices it remembers.
//! Phase 3: LAN sessions — [`engine::Exporter::prepare_send`] /
//! [`engine::Importer::connect`] over `carrova-transport`, with the wire
//! messages in [`lan`].
//!
//! The write-capable internals (`apply`, `stage`, `rollback`, `handlers`,
//! `journal`) are private: front-ends can only write through
//! [`engine::Importer`], which enforces plan approval. The `test-support`
//! feature exposes them to this crate's own integration tests through
//! one facade, `test_support`.

pub mod about;
mod apply;
pub mod cancel;
pub mod capture;
pub mod catalog;
pub mod checklist;
pub mod devices;
pub mod engine;
mod error;
pub mod export;
mod fsx;
mod handlers;
pub mod inbox;
mod journal;
pub mod kdf;
pub mod key;
pub mod lan;
mod mode;
pub mod plan;
pub mod presence;
mod rollback;
pub mod scan;
pub mod secrets;
pub mod select;
pub mod session;
mod space;
mod stage;
pub mod store;
pub mod time;
pub mod transfer_control;

/// The private write-capable internals, for this crate's own integration
/// tests only (feature `test-support`; front-ends must not enable it).
#[cfg(feature = "test-support")]
pub mod test_support {
    /// See the private `apply` module.
    pub mod apply {
        pub use crate::apply::*;
    }
    /// See the private `handlers` module.
    pub mod handlers {
        pub use crate::handlers::*;
    }
    /// See the private `journal` module.
    pub mod journal {
        pub use crate::journal::*;
    }
    /// See the private `rollback` module.
    pub mod rollback {
        pub use crate::rollback::*;
    }
    /// See the private `stage` module.
    pub mod stage {
        pub use crate::stage::*;
    }
}

#[cfg(feature = "failpoints")]
pub use apply::Failpoint;
pub use cancel::Cancel;
/// Archive/crypto types front-ends need, so they need not depend on those crates.
pub use carrova_archive::{ArchiveSummary, WriteOptions};
pub use carrova_crypto::{KdfParams, Passphrase};
pub use carrova_manifest::Manifest;
pub use carrova_plugin::{Plugin, PluginPackage};
/// Recipe model, re-exported so front-ends need not depend on `carrova-recipe`.
pub use carrova_recipe::Recipe;
/// LAN ticket (code + optional address) front-ends parse from user input.
pub use carrova_transport::Ticket;
pub use catalog::{Catalog, CatalogError, ValidationReport, load_catalog, validate_targets};
pub use checklist::{Checklist, Skipped};
pub use engine::{
    CodeOffer, Engine, Exporter, Importer, PlanApproval, SendHandle, SendOutcome, SendPrompt,
    SendReport, sessions,
};
pub use error::{
    CoreError, Corruption, EncodeError, ErrorClass, ErrorKind, Hint, JournalProblem,
    KeyFileProblem, Result, SessionProblem,
};
pub use inbox::{Inbox, Notice, NoticeId, NoticeKind, RETENTION_DAYS};
pub use kdf::KdfStrength;
pub use key::{ArtifactId, ItemKey, ItemTarget, RootedPath};
pub use plan::{
    Action, ConflictRule, ObjectKind, Plan, PlanEdit, PlanOptions, PlanOverride, PlanWarning,
    PlannedAction, PlannedPlugin, Remap, Retention,
};
pub use rollback::{Quarantined, RollbackFailure, RollbackPreview, RollbackReport};
pub use scan::{ScanError, ScanOptions, ScanReport, scan};
pub use select::{
    ItemChoice, Pick, PickFacts, PickKind, PickOutcome, PickRejection, PickResult, Preset,
    Selection, SelectionEdit, SelectionIssue,
};
pub use session::{
    Event, EventSink, LogSink, NullSink, PauseReason, Phase, Role, RollbackOutcome, SessionId,
    State, VecSink,
};
pub use store::{CorruptSession, SessionRecord, Source, Store};
pub use transfer_control::TransferControl;

// Front-ends drive sessions from worker threads: every session object must
// move across threads.
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Engine>();
    assert_send::<Exporter>();
    assert_send::<Importer>();
    assert_send::<SendHandle>();
};
