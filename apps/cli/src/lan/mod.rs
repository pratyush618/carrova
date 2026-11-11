//! `carrova send | receive`: LAN migration. Selection, plan review and apply
//! are the same flows as `export`/`import` ([`crate::session`]); this module
//! only adds pairing (showing / asking for the code) and resume.
//!
//! # The pairing code
//! The code is shown once per code on the sender's terminal (stderr). With
//! `--json` it appears instead as an explicit compact stdout record
//! `{"pairing_code": …, "ticket": …, "renewed": …}` — one line per code,
//! before the final result object — so scripts can read it. It never goes
//! through events, the session log or tracing.
//!
//! # Wrong codes
//! After a failed pairing attempt the engine asks ([`SendPrompt`](carrova_core::SendPrompt)) before
//! offering a new code: `carrova send` shows a loud warning and asks y/N
//! (default No; `--yes` does not answer it). Without a terminal, or with
//! `--json`, nobody can be asked, so no new code is offered and the send
//! stops. The engine stops the send after
//! [`carrova_core::lan::MAX_PAIRING_FAILURES`] wrong codes regardless.

mod send_ui;

use std::io::{IsTerminal, Write};
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, Result};
use carrova_core::lan::browse_nearby;
use carrova_core::session::{Role, SessionId};
use carrova_core::{
    Cancel, CoreError, Engine, Exporter, Importer, SendHandle, SendOutcome, SendReport, Ticket,
};
use carrova_view::lan::nearby_peer;
use clap::Args;
use serde_json::json;

use crate::args::{TicketParser, parse_session_id};
use crate::events::CliSink;
use crate::interrupt;
use crate::output::{OutputFormat, print_json};
use crate::prompt::{Consent, confirm, ensure_can_confirm, pairing_ticket};
use crate::recipes::RecipeLocations;
use crate::render::lan::{nearby, recovery_hint};
use crate::render::session::{ChecklistOf, checklist};
use crate::render::text::human_size;
use crate::session::{PlanArgs, SelectionArgs, choose, engine, review_and_apply, start_exporter};
use send_ui::SendUi;

/// `carrova send`
#[derive(Debug, Args)]
pub struct SendArgs {
    #[command(flatten)]
    selection: SelectionArgs,
    /// Continue an interrupted send (default: the latest one) with the
    /// items chosen then; the receiver reconnects without a new code.
    #[arg(
        long,
        value_name = "SESSION",
        num_args = 0..=1,
        value_parser = parse_session_id,
        conflicts_with_all = [
            "paths", "preset", "include", "exclude", "exclude_path",
            "confirm_sensitive", "force_unmovable", "allow_plugin",
        ],
    )]
    resume: Option<Option<SessionId>>,
    /// Don't ask for confirmation (required when stdin is not a terminal).
    /// Never answers "show a new code?" after a wrong code: without a
    /// terminal the send stops instead.
    #[arg(short, long)]
    yes: bool,
    /// Events as JSON lines on stderr; on stdout one compact
    /// `{"pairing_code": …}` line per code (for scripts), then the result.
    /// A wrong code entered on the other machine stops the send.
    #[arg(long)]
    json: bool,
}

/// `carrova receive`
#[derive(Debug, Args)]
pub struct ReceiveArgs {
    /// The code shown on the sending machine, or the `<code>@<ip>:<port>`
    /// line shown there when this machine can't find it. Asked for if
    /// omitted (preferred: arguments are visible to other local users).
    #[arg(value_name = "CODE|TICKET", value_parser = TicketParser, conflicts_with = "resume")]
    ticket: Option<Ticket>,
    /// Continue an interrupted receive (default: the latest one). Staged
    /// data is kept; an approved plan continues as approved when the
    /// options given here are the same, otherwise it is planned again
    /// with them and needs approval.
    #[arg(long, value_name = "SESSION", num_args = 0..=1, value_parser = parse_session_id)]
    resume: Option<Option<SessionId>>,
    #[command(flatten)]
    plan: PlanArgs,
}

impl SendArgs {
    fn format(&self) -> OutputFormat {
        OutputFormat::from_json_flag(self.json)
    }

    /// `--yes`: ask, or agreed up front.
    fn consent(&self) -> Consent {
        self.yes.into()
    }
}

/// `carrova send`: choose (or resume), pair and send, then report what the
/// receiver did.
pub fn send(
    out: &mut impl Write,
    recipes: &RecipeLocations,
    verbose: u8,
    a: &SendArgs,
) -> Result<ExitCode> {
    if a.resume.is_none() {
        ensure_can_confirm(a.consent(), "send")?;
    }
    let sink = Arc::new(CliSink::new(a.format(), verbose));
    let engine = engine(recipes, sink.clone())?;
    let started = match &a.resume {
        Some(given) => Some(resume_send(engine, given.as_ref(), a.format())?),
        None => start_send(out, engine, a)?,
    };
    let Some((handle, id)) = started else {
        eprintln!("send cancelled; nothing sent");
        return Ok(ExitCode::FAILURE);
    };
    sink.set_session(&id);
    let report = run_send(out, handle, &id, a.format())?;
    sent(out, &id, &report, a.format())?;
    Ok(ExitCode::SUCCESS)
}

/// A new send: select, confirm, prepare. `None` when the user declines.
fn start_send(
    out: &mut impl Write,
    engine: Engine,
    a: &SendArgs,
) -> Result<Option<(SendHandle, SessionId)>> {
    let mut exporter = start_exporter(engine, a.format(), a.selection.is_manual())?;
    let n = choose(out, &mut exporter, &a.selection, a.format())?;
    if !confirm(a.consent(), &format!("Send {n} items to another machine?"))? {
        return Ok(None);
    }
    let handle = {
        let _ctrl_c = interrupt::arm(&exporter.canceller(), a.format());
        exporter.prepare_send()?
    };
    Ok(Some((handle, exporter.id().clone())))
}

/// `--resume`: the interrupted send `given` (default: the latest).
fn resume_send(
    engine: Engine,
    given: Option<&SessionId>,
    format: OutputFormat,
) -> Result<(SendHandle, SessionId)> {
    let handle = with_hint(
        Exporter::resume_send(engine, given).map_err(anyhow::Error::new),
        given,
        Role::Export,
    )?;
    let id = handle.id().clone();
    if format == OutputFormat::Human {
        eprintln!("Waiting for the other machine to reconnect (session {id})...");
    }
    Ok((handle, id))
}

/// Pair and transfer, asking on the terminal when the engine needs a
/// decision (only interactive human output can answer).
fn run_send(
    out: &mut impl Write,
    handle: SendHandle,
    id: &SessionId,
    format: OutputFormat,
) -> Result<SendReport> {
    let interactive = format == OutputFormat::Human && std::io::stdin().is_terminal();
    let ask = interactive.then_some(|q: &str| confirm(Consent::Ask, q));
    let mut ui = SendUi::new(&mut *out, std::io::stderr(), format, ask);
    let _ctrl_c = interrupt::arm(&handle.canceller(), format);
    with_hint(
        handle.run(&mut ui).map_err(anyhow::Error::new),
        Some(id),
        Role::Export,
    )
    .with_context(|| format!("send session {id} did not complete"))
}

/// The send result: what was sent and the receiver's checklist.
fn sent(
    out: &mut impl Write,
    id: &SessionId,
    report: &SendReport,
    format: OutputFormat,
) -> Result<()> {
    let status = match report.outcome {
        SendOutcome::Applied => "sent",
        SendOutcome::DryRun => "dry_run",
    };
    match (format, report.outcome) {
        (OutputFormat::Json, _) => print_json(
            out,
            &json!({
                "session": id,
                "status": status,
                "bytes": report.bytes,
                "chunks": report.chunks,
                "checklist": report.checklist,
            }),
        )?,
        (OutputFormat::Human, SendOutcome::DryRun) => writeln!(
            out,
            "The other machine did a dry run; nothing was transferred."
        )?,
        (OutputFormat::Human, SendOutcome::Applied) => {
            writeln!(
                out,
                "Sent {} ({} chunks), session {id}",
                human_size(report.bytes),
                report.chunks
            )?;
            checklist(out, &report.checklist, ChecklistOf::Receiver)?;
        }
    }
    Ok(())
}

/// `carrova receive`
pub fn receive(
    out: &mut impl Write,
    recipes: &RecipeLocations,
    verbose: u8,
    a: &ReceiveArgs,
) -> Result<ExitCode> {
    let p = &a.plan;
    if p.needs_confirmation() {
        ensure_can_confirm(p.consent(), "receive")?;
    }
    let sink = Arc::new(CliSink::new(p.format(), verbose));
    let engine = engine(recipes, sink.clone())?;
    let platform = Arc::clone(engine.platform());
    let mut importer = open_receive(engine, a)?;
    let id = importer.id().clone();
    sink.set_session(&id);
    with_hint(
        review_and_apply(out, &mut importer, platform.as_ref(), p, verbose),
        Some(&id),
        Role::Import,
    )
}

/// Before asking for the code: the senders nearby offering one (best
/// effort; a failed look says nothing).
fn show_nearby(platform: &dyn carrova_platform::Platform) {
    eprintln!("Looking for machines nearby...");
    match browse_nearby(platform) {
        Ok(found) => {
            let peers: Vec<_> = found.iter().map(nearby_peer).collect();
            eprintln!("{}", nearby(&peers));
        }
        Err(e) => tracing::debug!(error = %e, "looking for senders nearby failed"),
    }
}

/// The receive session: resumed, or paired with the ticket (asked for if
/// not given). Ctrl-C cancels the wait for the sender.
fn open_receive(engine: Engine, a: &ReceiveArgs) -> Result<Importer> {
    let cancel = Cancel::default();
    if let Some(given) = &a.resume {
        let _ctrl_c = interrupt::arm(&cancel, a.plan.format());
        let resumed = Importer::resume_lan_with(engine, given.as_ref(), cancel);
        return with_hint(
            resumed.map_err(anyhow::Error::new),
            given.as_ref(),
            Role::Import,
        )
        .context("resuming the receive");
    }
    let ticket = match &a.ticket {
        Some(t) => t.clone(),
        None => {
            if std::io::stdin().is_terminal() {
                show_nearby(engine.platform().as_ref());
            }
            pairing_ticket()?
        }
    };
    let _ctrl_c = interrupt::arm(&cancel, a.plan.format());
    Importer::connect_with(engine, &ticket, cancel).map_err(|e| match e {
        CoreError::Cancelled => anyhow::Error::new(e),
        e => anyhow::Error::new(e).context(
            "could not pair with the sending machine; check the code it shows now (a wrong \
             attempt replaces it) or type the `<code>@<ip>:<port>` line shown there",
        ),
    })
}

/// `r`, with [`recovery_hint`]'s advice on top when it failed with a LAN
/// error that has one.
fn with_hint<T>(r: Result<T>, id: Option<&SessionId>, role: Role) -> Result<T> {
    r.map_err(|e| {
        let hint = e
            .downcast_ref::<CoreError>()
            .and_then(|c| recovery_hint(c, id, role));
        match hint {
            Some(hint) => e.context(hint),
            None => e,
        }
    })
}

#[cfg(test)]
#[path = "../../tests/lan/unit.rs"]
mod tests;
