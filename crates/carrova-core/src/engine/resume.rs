//! Continuing an import after a restart (archive here, LAN in
//! `receive.rs`): crash recovery of an interrupted apply, or a re-plan
//! against the current filesystem. Per-state semantics are in the module
//! docs of [`crate::engine`].

use carrova_crypto::Passphrase;

use super::Engine;
use super::archive::ArchiveSource;
use super::import::{Approval, ImportFrom, Importer};
use super::session::Session;
use crate::cancel::Cancel;
use crate::error::{CoreError, Result, SessionProblem};
use crate::plan::Plan;
use crate::rollback::{self, RollbackReport};
use crate::session::{Role, SessionId, State};
use crate::store::PLAN_FILE;

/// Where a restarted import continues, by its persisted state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResumePoint {
    /// `Applying`: roll the interrupted apply back; the session ends.
    Recover,
    /// `Selected` … `Staged`: plan again from this state.
    Replan(State),
}

impl ResumePoint {
    /// The resume point of `s`; terminal (and `Scanned`) sessions have
    /// nothing to resume.
    pub(super) fn of(s: &Session) -> Result<Self> {
        match s.record.state {
            State::Applying => Ok(Self::Recover),
            state @ (State::Selected | State::Planned | State::Transferring | State::Staged) => {
                Ok(Self::Replan(state))
            }
            state @ (State::Done | State::RolledBack | State::Failed | State::Scanned) => {
                Err(s.problem(SessionProblem::Ended(state)))
            }
        }
    }
}

impl Session {
    /// Crash recovery from `Applying`: replay the journal (if any), wipe
    /// staging, end `RolledBack` (or `Failed` when some step needs manual
    /// attention).
    pub(super) fn recover_interrupted_apply(&mut self) -> Result<()> {
        let report = if self.has_journal()? {
            rollback::rollback(self.platform(), &self.dir, self.sink())?
        } else {
            RollbackReport::default()
        };
        self.wipe_staging();
        if report.failed.is_empty() {
            self.record.error = Some("apply was interrupted; changes were rolled back".into());
            self.transition(State::RolledBack)
        } else {
            self.fail(&CoreError::RollbackFailed {
                cause: "apply was interrupted".into(),
                rollback: report.failure_summary(),
            });
            Ok(())
        }
    }

    /// `result`, failing the session first if it is a
    /// [`CoreError::Verification`] error (the source changed since the
    /// session started, so it can never continue).
    pub(super) fn unless_changed<T>(&mut self, result: Result<T>) -> Result<T> {
        result.inspect_err(|e| {
            if matches!(e, CoreError::Verification(_)) {
                self.fail(e);
            }
        })
    }
}

impl Importer {
    /// Resume a persisted archive import session (re-asks the passphrase).
    /// See the module docs of [`crate::engine`] for per-state semantics.
    pub fn resume_archive(engine: Engine, id: &SessionId, pass: &Passphrase) -> Result<Self> {
        let mut s = Session::load(engine, id)?;
        if s.record.role != Role::Import {
            return Err(s.problem(SessionProblem::ExportNotResumable));
        }
        let Some(path) = s.record.archive_path() else {
            return Err(s.problem(SessionProblem::NotArchiveImport));
        };
        let archive = ArchiveSource::new(path, pass);
        let old_plan: Option<Plan> = s.read_json(PLAN_FILE)?;
        match ResumePoint::of(&s)? {
            ResumePoint::Recover => {
                // Recover first: needs neither archive nor passphrase.
                s.recover_interrupted_apply()?;
                let manifest = archive
                    .manifest(s.platform(), s.record.archive_fingerprint())
                    .ok();
                Ok(Self::finished(
                    s,
                    ImportFrom::Archive(archive),
                    old_plan,
                    manifest,
                    Cancel::default(),
                ))
            }
            ResumePoint::Replan(state) => {
                s.wipe_staging();
                let manifest = archive.manifest(s.platform(), s.record.archive_fingerprint());
                let manifest = s.unless_changed(manifest)?;
                let mut imp =
                    Self::resumed(s, ImportFrom::Archive(archive), manifest, Cancel::default());
                imp.replan_after_restart(state, old_plan)?;
                Ok(imp)
            }
        }
    }

    /// After a restart in `state` (`Selected` … `Staged`): recompute the
    /// plan from the persisted options against the current filesystem
    /// (state `Planned`), keeping the approval only if actions and
    /// handlers are unchanged and nothing blocks; without a persisted plan,
    /// back to `Selected`. Shared by archive and LAN resume.
    pub(super) fn replan_after_restart(
        &mut self,
        state: State,
        old_plan: Option<Plan>,
    ) -> Result<()> {
        if state == State::Selected {
            return Ok(());
        }
        let Some(old) = old_plan else {
            // No plan was persisted: back to choosing.
            self.s.record.approved = false;
            self.s.transition(State::Planned)?;
            return self.s.transition(State::Selected);
        };
        let new = self.compute_plan(&old.options)?;
        let unchanged = self.s.record.approved
            && new.blockers.is_empty()
            && new.actions == old.actions
            && new.handlers == old.handlers;
        let approval = if unchanged {
            Approval::Keep
        } else {
            Approval::Reset
        };
        self.install_plan(new, approval)?;
        Ok(())
    }
}
