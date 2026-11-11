//! The receiving side of `import` and `receive`: plan, review, confirm,
//! apply. Each step is its own function; [`review_and_apply`] sequences
//! them.

use std::io::Write;
use std::process::ExitCode;

use anyhow::{Result, bail};
use carrova_core::plan::{Plan, PlanEdit};
use carrova_core::session::{Role, SessionId, State};
use carrova_core::{CoreError, ErrorKind, Importer, PlanApproval};
use carrova_platform::Platform;
use carrova_view::review::{self, Prepared};
use serde_json::{Value, json};

use super::PlanArgs;
use crate::interrupt;
use crate::output::{OutputFormat, print_json};
use crate::prompt::{Consent, confirm};
use crate::render::plan::plan as render_plan;
use crate::render::session::{ChecklistOf, checklist};
use carrova_view::Detail;

/// From a session in state `Selected` (new or resumed) or `Planned`: plan
/// with its options as changed by `a`, show the plan, stop on blockers or
/// `--dry-run`, ask, approve, apply.
///
/// A resumed session is planned from its own options, changed only by
/// the flags passed in `a`; the shared review step
/// ([`carrova_view::review::prepare`]) keeps its approved plan when that
/// changes nothing, and it is then applied without asking again. A resumed session that already ended (e.g. an interrupted apply
/// rolled back during resume) is only reported.
pub fn review_and_apply(
    out: &mut impl Write,
    importer: &mut Importer,
    platform: &dyn Platform,
    a: &PlanArgs,
    verbose: u8,
) -> Result<ExitCode> {
    let id = importer.id().clone();
    let state = importer.state();
    let format = a.format();
    match state {
        State::Selected | State::Planned => {}
        State::Done | State::RolledBack | State::Failed => {
            return ended(out, &id, state, format);
        }
        State::Scanned | State::Transferring | State::Staged | State::Applying => {
            bail!("session {id} is {state}; it cannot be continued here");
        }
    }
    let review = Review::prepare(importer, platform, a, id)?;
    review.show(out, importer, Detail::from_verbosity(verbose))?;
    review.check_blockers(out)?;
    if a.dry_run() {
        return review.dry_run(out, importer);
    }
    if !review.confirm(out, importer, a.consent())? {
        eprintln!("import cancelled; nothing written");
        return Ok(ExitCode::FAILURE);
    }
    #[cfg(feature = "failpoints")]
    a.arm_failpoint(importer);
    review.apply(out, importer)
}

/// A resumed session that had already ended: report its state.
fn ended(
    out: &mut impl Write,
    id: &SessionId,
    state: State,
    format: OutputFormat,
) -> Result<ExitCode> {
    match format {
        OutputFormat::Json => print_json(out, &json!({ "session": id, "status": state }))?,
        OutputFormat::Human => writeln!(
            out,
            "Session {id} is {state}; nothing more to do (see `carrova sessions show {id}`)."
        )?,
    }
    Ok(if state == State::Done {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// The plan under review and what is known about it.
struct Review {
    id: SessionId,
    /// The plan, the space it needs and whether it still needs approval.
    prepared: Prepared,
    format: OutputFormat,
}

impl Review {
    /// The plan for the session's options changed by the flags in `a`
    /// ([`PlanArgs::options_over`], [`carrova_view::review::prepare`]),
    /// planned again with "Accept all" under `--accept-safe-picks`
    /// ([`PlanEdit::AcceptSafePicks`]: each plan judges its own safe picks).
    fn prepare(
        importer: &mut Importer,
        platform: &dyn Platform,
        a: &PlanArgs,
        id: SessionId,
    ) -> Result<Self> {
        let mut options = a.options_over(importer.plan_options().cloned().unwrap_or_default());
        let mut prepared = review::prepare(importer, platform, &options)?;
        let offered = importer.manifest().map(|m| prepared.plan.offered(m));
        if a.accept_safe_picks()
            && let Some(offered) = offered
        {
            options
                .apply(PlanEdit::AcceptSafePicks, &offered)
                .map_err(CoreError::from)?;
            prepared = review::prepare(importer, platform, &options)?;
        }
        Ok(Self {
            id,
            prepared,
            format: a.format(),
        })
    }

    fn plan(&self) -> &Plan {
        &self.prepared.plan
    }

    /// The JSON result object: session, `status`, plan, plus `extra`.
    fn result(&self, status: &str, extra: Value) -> Value {
        let mut v = json!({ "session": self.id, "status": status, "plan": self.plan() });
        if let (Some(v), Value::Object(extra)) = (v.as_object_mut(), extra) {
            v.extend(extra);
        }
        v
    }

    /// The plan, in human output.
    fn show(&self, out: &mut impl Write, importer: &Importer, detail: Detail) -> Result<()> {
        if self.format == OutputFormat::Human {
            let manifest = importer.manifest();
            render_plan(out, self.plan(), manifest, &self.prepared.needs, detail)?;
        }
        Ok(())
    }

    /// Fail when the plan has blockers.
    fn check_blockers(&self, out: &mut impl Write) -> Result<()> {
        if self.plan().blockers.is_empty() {
            return Ok(());
        }
        if self.format == OutputFormat::Json {
            print_json(out, &self.result("blocked", json!({})))?;
        }
        bail!(
            "plan is blocked ({} blocker(s) above); resolve them, deselect items, or override \
             with --ignore-running / --ignore-free-space where offered",
            self.plan().blockers.len()
        );
    }

    /// `--dry-run`: end the session without writing.
    fn dry_run(&self, out: &mut impl Write, importer: &mut Importer) -> Result<ExitCode> {
        // LAN: tell the sender nothing will be transferred (no-op for archives).
        importer.end_dry_run()?;
        match self.format {
            OutputFormat::Json => print_json(out, &self.result("dry_run", json!({})))?,
            OutputFormat::Human => writeln!(out, "Dry run: nothing was written.")?,
        }
        Ok(ExitCode::SUCCESS)
    }

    /// Approve the plan, asking first unless it was approved earlier.
    /// `false` when the user declines.
    fn confirm(
        &self,
        out: &mut impl Write,
        importer: &mut Importer,
        consent: Consent,
    ) -> Result<bool> {
        if self.prepared.approval == PlanApproval::Given {
            if self.format == OutputFormat::Human {
                writeln!(out, "This plan was approved earlier; continuing.")?;
            }
            return Ok(true);
        }
        if !confirm(consent, "Apply this plan?")? {
            return Ok(false);
        }
        importer.approve()?;
        Ok(true)
    }

    /// Apply the approved plan and report the checklist or the failure.
    fn apply(&self, out: &mut impl Write, importer: &mut Importer) -> Result<ExitCode> {
        let applied = {
            // Cancels the transfer; ignored once apply writes (engine rule).
            let _ctrl_c = interrupt::arm(&importer.canceller(), self.format);
            importer.apply()
        };
        let list = match applied {
            Ok(list) => list,
            Err(e) => return Err(self.failed(out, e)?),
        };
        match self.format {
            OutputFormat::Json => {
                print_json(out, &self.result("applied", json!({ "checklist": list })))?;
            }
            OutputFormat::Human => {
                let of = ChecklistOf::Local {
                    session: &self.id.0,
                    role: Role::Import,
                };
                checklist(out, &list, of)?;
            }
        }
        Ok(ExitCode::SUCCESS)
    }

    /// Report a failed apply (JSON) and turn it into the command's error.
    fn failed(&self, out: &mut impl Write, e: CoreError) -> Result<anyhow::Error> {
        let status = match e.class().kind {
            ErrorKind::RolledBack => "rolled_back",
            _ => "failed",
        };
        if self.format == OutputFormat::Json {
            print_json(out, &self.result(status, json!({ "error": e.to_string() })))?;
        }
        let id = &self.id;
        Ok(anyhow::Error::new(e).context(format!("import session {id} did not complete")))
    }
}
