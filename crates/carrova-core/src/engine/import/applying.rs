//! Applying an approved import: stage (archive or LAN), plugins over the
//! staged files, the last cancel point, apply, and what a failed or cancelled staging run leaves behind.

use super::{Approval, ImportFrom, Importer};
use crate::apply;
use crate::checklist::Checklist;
use crate::engine::checklist::import_checklist;
use crate::engine::transfer::Disk;
use crate::error::{CoreError, Result};
use crate::lan::{LanMsg, ReceiverFailure};
use crate::plan::{Plan, Retention};
use crate::rollback;
use crate::session::{Event, State};
use crate::space;
use crate::stage::{self, StageLayout, Staged};
use crate::time;

/// What a failed staging run leaves behind.
enum StageFailure {
    /// Back to `Planned` with staging kept, so `apply` or a resume can
    /// continue.
    Resumable(Approval),
    /// Either side cancelled: back to `Planned` with the approval, the
    /// LAN link closed (the sender told if we cancelled), LAN staging
    /// kept; a resume continues.
    Cancelled,
    /// Staging wiped, session failed, the sender told.
    Fatal,
}

impl Importer {
    /// Stage + verify + apply (states `Transferring` → `Staged` → `Applying`
    /// → `Done`, or `RolledBack` on failure). Staging is always wiped; with
    /// [`Retention::DiscardOnSuccess`] the rollback point is dismissed after
    /// success; with [`Retention::KeepFor`] it expires that many days later
    /// ([`crate::engine::sessions::prune_expired`]).
    ///
    /// A cancel ([`Self::canceller`]) before `Applying` ends the call with
    /// [`CoreError::Cancelled`] and nothing written to the user's roots
    /// (state back to `Planned`, approval kept); once `Applying`, it is
    /// ignored until apply ends. The LAN sender cancelling is
    /// [`CoreError::PeerCancelled`], with the same outcome.
    ///
    /// Free space is re-checked by the planner's policy before anything is
    /// written and again before placement, unless the plan overrides it:
    /// a LAN import pauses until space frees up; an archive import fails
    /// with [`CoreError::NotEnoughSpace`] (before staging: still `Planned`
    /// and approved, nothing written).
    pub fn apply(&mut self) -> Result<Checklist> {
        let plan = self.approved_plan()?;
        // The approved plan's override lifts every runtime free-space guard
        // (chunk staging and plugin output) via the one live control.
        if plan.options.ignore_free_space {
            self.control.ignore_free_space();
        }
        self.cancel.check()?;
        // Nothing written yet: a disk that filled up since approval stops
        // (archive) or pauses (LAN) here. Chunks already staged by an
        // earlier LAN run occupy the state dir, which the transfer's own
        // per-batch check guards.
        let restaging = matches!(self.from, ImportFrom::Lan(_))
            && self.s.platform().metadata(&self.s.staging()).is_ok();
        if let Err(e) = self.wait_for_space(|| self.check_space(&plan, restaging)) {
            self.close_link_after(&e);
            return Err(e);
        }
        self.s.transition(State::Transferring)?;
        let staged = match self.stage(&plan) {
            Ok(staged) => staged,
            Err(e) => return Err(self.stage_failed(e)),
        };
        let applied = self.open_manifest(State::Applying).and_then(|manifest| {
            apply::apply(
                self.s.platform(),
                &plan,
                manifest,
                &staged,
                &self.s.dir,
                self.s.sink(),
                self.failpoint,
            )
        });
        self.s.wipe_staging();
        let outcome = match applied {
            Ok(_report) => self.finish(&plan).inspect_err(|e| self.s.fail(e)),
            Err(e @ CoreError::RolledBack { .. }) => {
                self.s.record.error = Some(e.to_string());
                if let Err(pe) = self.s.transition(State::RolledBack) {
                    tracing::warn!(error = %pe, "could not persist rolled-back state");
                }
                Err(e)
            }
            Err(e) => {
                self.s.fail(&e);
                Err(e)
            }
        };
        self.notify_sender(&match &outcome {
            Ok(checklist) => LanMsg::Applied(checklist.clone()),
            Err(CoreError::RolledBack { .. }) => ReceiverFailure::ApplyRolledBack.into(),
            Err(_) => ReceiverFailure::ApplyFailed.into(),
        });
        outcome
    }

    /// The plan `apply` may run: approved, unblocked, from `Planned`.
    fn approved_plan(&self) -> Result<Plan> {
        let plan = match &self.plan {
            Some(p) if self.s.record.approved => p,
            _ => return Err(CoreError::NotApproved),
        };
        plan.ensure_unblocked()?;
        let from = self.s.record.state;
        if from != State::Planned {
            return Err(CoreError::InvalidTransition {
                from,
                to: State::Transferring,
            });
        }
        Ok(plan.clone())
    }

    fn stage(&mut self, plan: &Plan) -> Result<Staged> {
        let staged = match &self.from {
            ImportFrom::Archive(archive) => {
                let platform = self.s.platform();
                let mut reader = archive.reopen(platform, self.s.record.archive_fingerprint())?;
                stage::stage(
                    platform,
                    &mut reader,
                    plan,
                    &self.s.staging(),
                    self.s.sink(),
                    &self.cancel,
                )?
            }
            ImportFrom::Lan(_) => self.stage_lan(plan)?,
        };
        let staged = self.run_plugins(plan, &staged)?;
        // Placement still needs room on targets apart from the state dir
        // (on its filesystem, staged files are renamed into place).
        self.wait_for_space(|| self.check_space(plan, true))?;
        // Last chance to stop: from `Applying` on, apply runs to the end.
        self.cancel.check()?;
        self.s.transition(State::Staged)?;
        self.s.transition(State::Applying)?;
        Ok(staged)
    }

    /// `staged` with the plan's plugins run over it. On a LAN import a full
    /// disk pauses like the chunk transfer does (the user's free-space
    /// override lifts the plugin guard too); an archive import fails.
    fn run_plugins(&self, plan: &Plan, staged: &Staged) -> Result<Staged> {
        let run = || {
            // Each attempt starts from the verified staging, so a retry
            // after a full disk redoes the same outputs.
            let mut out = staged.clone();
            stage::run_plugins(
                self.s.platform(),
                plan,
                self.open_manifest(State::Staged)?,
                self.s.engine.plugins(),
                &mut out,
                !self.control.ignores_free_space(),
            )?;
            Ok(out)
        };
        self.wait_for_space(run)
    }

    /// `op`, on a LAN import retried while it reports a full disk (paused
    /// like the chunk transfer); an archive import fails at once.
    fn wait_for_space<T>(&self, mut op: impl FnMut() -> Result<T>) -> Result<T> {
        match self.from {
            ImportFrom::Archive(_) => op(),
            ImportFrom::Lan(_) => {
                let staging = self.s.staging();
                let disk = Disk::new(
                    self.s.platform(),
                    &staging,
                    self.s.sink(),
                    self.s.store.root(),
                    &self.cancel,
                    &self.control,
                );
                disk.wait(op)
            }
        }
    }

    /// The planner's free-space policy ([`space::needs`]) re-run for
    /// `plan` now, skipping the state-dir filesystem if `skip_state_dir`:
    /// [`CoreError::NotEnoughSpace`] for the first short filesystem,
    /// unless the user lifted the guard.
    fn check_space(&self, plan: &Plan, skip_state_dir: bool) -> Result<()> {
        if self.control.ignores_free_space() {
            return Ok(());
        }
        let manifest = self.open_manifest(State::Transferring)?;
        let layout = StageLayout::new(plan, manifest)?;
        let mut needs = space::needs(self.s.platform(), plan, &layout);
        needs.retain(|n| !(skip_state_dir && n.state_dir));
        space::ensure_room(self.s.platform(), &needs)
    }

    /// Staging failed with `e`; see [`Self::stage_failure`] for what is
    /// kept. Returns `e`.
    fn stage_failed(&mut self, e: CoreError) -> CoreError {
        match self.stage_failure(&e) {
            StageFailure::Resumable(approval) => {
                self.s.record.approved = approval == Approval::Keep;
                if let Err(pe) = self.s.transition(State::Planned) {
                    tracing::warn!(error = %pe, "could not persist planned state");
                }
            }
            StageFailure::Cancelled => {
                self.close_link_after(&e);
                if matches!(self.from, ImportFrom::Archive(_)) {
                    // Restaged from the archive on resume anyway.
                    self.s.wipe_staging();
                }
                if let Err(pe) = self.s.transition(State::Planned) {
                    tracing::warn!(error = %pe, "could not persist planned state");
                }
            }
            StageFailure::Fatal => {
                self.s.wipe_staging();
                self.s.fail(&e);
                self.notify_sender(&ReceiverFailure::StageFailed.into());
            }
        }
        e
    }

    /// Close the LAN link after the cancel `e`: the sender is told when
    /// this side cancelled; when the sender did, its connection is gone.
    fn close_link_after(&mut self, e: &CoreError) {
        if let ImportFrom::Lan(link) = &mut self.from
            && let Some(link) = link.take()
            && matches!(e, CoreError::Cancelled)
        {
            link.cancelled();
        }
    }

    /// A cancel on either side is [`StageFailure::Cancelled`]. An archive
    /// import (and a LAN import with a local failure) wipes
    /// staging and fails; the sender is told. A LAN import that lost its
    /// sender or was missing items keeps its staging and goes back to
    /// `Planned` (approval kept only after a lost connection), so `apply`
    /// or [`Self::resume_lan`] can continue. Failed dials while
    /// reconnecting never get here: the link retries them until its window
    /// ends ([`CoreError::PeerUnreachable`]), so a stranger answering
    /// discovery cannot wipe staging or the resume key.
    fn stage_failure(&self, e: &CoreError) -> StageFailure {
        if matches!(e, CoreError::Cancelled | CoreError::PeerCancelled) {
            return StageFailure::Cancelled;
        }
        if !matches!(self.from, ImportFrom::Lan(_)) {
            return StageFailure::Fatal;
        }
        match e {
            CoreError::TransferIncomplete { .. } => StageFailure::Resumable(Approval::Reset),
            CoreError::PeerUnreachable { .. } => StageFailure::Resumable(Approval::Keep),
            e if e.is_connection_lost() => StageFailure::Resumable(Approval::Keep),
            _ => StageFailure::Fatal,
        }
    }

    fn finish(&mut self, plan: &Plan) -> Result<Checklist> {
        if plan.options.retention == Retention::DiscardOnSuccess
            && let Err(e) = rollback::dismiss(self.s.platform(), &self.s.dir)
        {
            tracing::warn!(error = %e, "could not discard rollback point");
            self.s.emit(Event::Warning {
                message: format!("could not discard the rollback point: {e}"),
            });
        }
        let rollback_available = self.s.has_journal()?;
        let checklist = import_checklist(
            self.open_manifest(State::Done)?,
            plan,
            self.s.engine.recipes(),
            rollback_available,
        );
        self.s.record.checklist = Some(checklist.clone());
        self.s.record.bytes_moved = Some(plan.bytes_to_write);
        self.s.record.expires_at = match plan.options.retention {
            Retention::KeepFor { days } => {
                Some(time::now_unix().saturating_add(u64::from(days) * time::SECS_PER_DAY))
            }
            Retention::UntilDismissed | Retention::DiscardOnSuccess => None,
        };
        self.s.record.error = None;
        self.s.transition(State::Done)?;
        self.s.emit(Event::Finished {
            checklist: checklist.clone(),
        });
        Ok(checklist)
    }
}
