//! Receiver session: archive or LAN source → plan → approve → stage →
//! apply. Staging and applying live in `applying.rs`, the LAN specifics in
//! `receive.rs`, restarts in `resume.rs`.

use std::path::Path;

use carrova_archive::ArchiveReader;
use carrova_crypto::Passphrase;
use carrova_manifest::Manifest;

mod applying;
mod review;
mod source;

pub use review::PlanApproval;
pub(super) use source::{ImportFrom, Loaded};

use super::Engine;
use super::archive::{ArchiveSource, fingerprint};
use super::redact::Patterns;
use super::session::{Session, absolute};
use crate::apply::Failpoint;
use crate::cancel::Cancel;
use crate::error::{CoreError, Result};
use crate::key::RootedPath;
use crate::lan::{self, LanMsg, PlanSummary, ReceiverStage};
use crate::plan::{self, Plan, PlanOptions};
use crate::session::{Event, Role, SessionId, State};
use crate::store::{PLAN_FILE, SessionRecord, Source};
use crate::transfer_control::TransferControl;

/// Receiver session (archive or LAN). For archives the passphrase is kept
/// in memory only (zeroized on drop) to reopen the archive for staging; it
/// is never persisted or logged.
#[derive(Debug)]
pub struct Importer {
    pub(super) s: Session,
    pub(super) from: ImportFrom,
    pub(super) manifest: Loaded,
    pub(super) plan: Option<Plan>,
    /// Stops [`Self::approve`] (LAN) and [`Self::apply`] before apply
    /// starts writing; see [`crate::cancel`].
    pub(super) cancel: Cancel,
    /// The user's pause / free-space override of a LAN transfer; see
    /// [`crate::transfer_control`].
    pub(super) control: TransferControl,
    /// Test-only fault injection into apply (zero-sized, inert, outside
    /// feature `failpoints`).
    pub(super) failpoint: Failpoint,
}

/// Whether installing a plan keeps the user's approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Approval {
    /// The plan counts as approved (the user approved an identical one).
    Keep,
    /// The user must approve it.
    Reset,
}

impl Importer {
    /// Open + authenticate an archive, start a session (state `Selected`:
    /// the archive's items are the candidates).
    pub fn open(engine: Engine, archive: &Path, pass: &Passphrase) -> Result<Self> {
        let reader = ArchiveReader::open(&**engine.platform(), archive, pass)?;
        let fingerprint = fingerprint(&reader)?;
        let manifest = reader.manifest().clone();
        drop(reader);
        let path = absolute(archive);
        let source = ArchiveSource::new(&path, pass);
        let s = Session::create(
            engine,
            Role::Import,
            State::Selected,
            Some(Source::Archive {
                path,
                fingerprint: Some(fingerprint),
            }),
        )?;
        Self::started(s, ImportFrom::Archive(source), manifest, Cancel::default())
    }

    /// A new import session with its manifest: records the manifest's
    /// sensitive paths for log redaction. Shared by archive and LAN;
    /// `cancel` is the one its LAN link (if any) already watches.
    pub(super) fn started(
        s: Session,
        from: ImportFrom,
        manifest: Manifest,
        cancel: Cancel,
    ) -> Result<Self> {
        s.add_redactions(Patterns::new(s.platform()).manifest(&manifest))?;
        Ok(Self::resumed(s, from, manifest, cancel))
    }

    /// A persisted session continuing with its manifest (no plan yet).
    pub(super) fn resumed(
        s: Session,
        from: ImportFrom,
        manifest: Manifest,
        cancel: Cancel,
    ) -> Self {
        Self {
            s,
            from,
            manifest: Loaded::Open(manifest),
            plan: None,
            cancel,
            control: TransferControl::default(),
            failpoint: Failpoint::OFF,
        }
    }

    /// A session that ended while resuming (see [`Loaded::Finished`]).
    pub(super) fn finished(
        s: Session,
        from: ImportFrom,
        plan: Option<Plan>,
        manifest: Option<Manifest>,
        cancel: Cancel,
    ) -> Self {
        Self {
            s,
            from,
            manifest: Loaded::Finished(manifest),
            plan,
            cancel,
            control: TransferControl::default(),
            failpoint: Failpoint::OFF,
        }
    }

    /// Cancels this session's running (or next) blocking call from another
    /// thread: take it before calling [`Self::approve`] or [`Self::apply`].
    /// Apply ignores it once it starts writing (it completes or rolls
    /// back); see [`crate::cancel`].
    #[must_use]
    pub fn canceller(&self) -> Cancel {
        self.cancel.clone()
    }

    /// Pauses, resumes or lifts the free-space guard of a running LAN
    /// transfer ([`Self::apply`]) from another thread: take it before
    /// calling `apply`. No effect on archive imports.
    #[must_use]
    pub fn transfer_control(&self) -> TransferControl {
        self.control.clone()
    }

    /// Session id.
    #[must_use]
    pub fn id(&self) -> &SessionId {
        self.s.id()
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> State {
        self.s.record.state
    }

    /// Persisted record (state, approval, checklist, error).
    #[must_use]
    pub fn record(&self) -> &SessionRecord {
        &self.s.record
    }

    /// Manifest of the archive or sender; `None` only for a session that
    /// ended while resuming and whose source can no longer be read.
    #[must_use]
    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.get()
    }

    /// Make the next [`Self::apply`] fail at `failpoint` (and roll back):
    /// fault injection for tests, only in `failpoints` builds.
    #[cfg(feature = "failpoints")]
    pub fn set_failpoint(&mut self, failpoint: Failpoint) {
        self.failpoint = failpoint;
    }

    /// Current plan, if computed.
    #[must_use]
    pub fn current_plan(&self) -> Option<&Plan> {
        self.plan.as_ref()
    }

    /// Dry run (state `Planned`). Re-callable with new options; writes nothing
    /// outside the session dir. A new plan always needs a new approval.
    pub fn plan(&mut self, options: &PlanOptions) -> Result<&Plan> {
        let from = self.s.record.state;
        if !matches!(from, State::Selected | State::Planned) {
            return Err(CoreError::InvalidTransition {
                from,
                to: State::Planned,
            });
        }
        let plan = self.compute_plan(options)?;
        self.install_plan(plan, Approval::Reset)
    }

    /// User approves the current plan. Fails while it has blockers. A LAN
    /// import first sends the sender a [`crate::lan::PlanSummary`]
    /// (reconnecting if needed); the approval is only recorded once it
    /// was delivered; a cancel ([`Self::canceller`]) stops a reconnect
    /// with [`CoreError::Cancelled`] and records nothing.
    pub fn approve(&mut self) -> Result<()> {
        let plan = match (&self.plan, self.s.record.state) {
            (Some(p), State::Planned) => p,
            (_, from) => {
                return Err(CoreError::InvalidTransition {
                    from,
                    to: State::Transferring,
                });
            }
        };
        plan.ensure_unblocked()?;
        if let ImportFrom::Lan(link) = &mut self.from {
            self.cancel.check()?;
            link.as_mut().ok_or(CoreError::LanEnded)?.send(
                &LanMsg::PlanSummary(PlanSummary::from_plan(plan)),
                ReceiverStage::Planning,
                lan::RECONNECT_WINDOW,
                self.s.sink(),
            )?;
        }
        self.s.record.approved = true;
        self.s.save()
    }

    /// The manifest to move on to `to` with (see [`Loaded::open`]).
    pub(super) fn open_manifest(&self, to: State) -> Result<&Manifest> {
        self.manifest.open(self.s.record.state, to)
    }

    pub(super) fn compute_plan(&self, options: &PlanOptions) -> Result<Plan> {
        plan::plan_with_plugins(
            self.s.platform(),
            self.s.engine.recipes(),
            self.s.engine.plugins(),
            self.open_manifest(State::Planned)?,
            options,
            self.s.id(),
            &crate::time::date(crate::time::now_unix()),
        )
    }

    /// Persist `plan` (approval reset first, so a crash never leaves an
    /// old approval attached to a new plan), move to `Planned`, report
    /// conflicts. Plan warnings are not emitted as events: they travel with
    /// the returned / persisted plan, which front-ends render once.
    pub(super) fn install_plan(&mut self, plan: Plan, approval: Approval) -> Result<&Plan> {
        if self.s.record.approved {
            self.s.record.approved = false;
            self.s.save()?;
        }
        self.s
            .add_redactions(Patterns::new(self.s.platform()).plan(&plan, self.manifest.get()))?;
        self.s.write_json(PLAN_FILE, &plan)?;
        self.s.record.approved = approval == Approval::Keep;
        self.s.transition(State::Planned)?;
        for a in plan.conflicts() {
            self.s.emit(Event::ConflictFound {
                item: a.item.clone(),
                target: RootedPath::new(a.root, &a.rel).to_string(),
            });
        }
        Ok(self.plan.insert(plan))
    }
}
