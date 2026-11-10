//! `carrova report list | show | issue | delete | clear | consent`.

use std::io::Write;
use std::process::ExitCode;

use anyhow::Result;
use carrova_report::{ReportConsent, ReportId, ReportStore, issue_draft};
use serde_json::json;

use super::Reports;
use super::args::{ReportArgs, ReportCommand};
use crate::output::{OutputFormat, print_json};
use crate::prompt::{confirm, ensure_can_confirm};
use crate::render::report as render;

/// `carrova report`
pub fn run(out: &mut impl Write, reports: &Reports, a: ReportArgs) -> Result<ExitCode> {
    let format = OutputFormat::from_json_flag(a.json);
    let store = &reports.store;
    match a.command.unwrap_or(ReportCommand::List) {
        ReportCommand::List => list(out, store, format)?,
        ReportCommand::Show { id } => {
            let report = store.read(&id)?;
            match format {
                OutputFormat::Json => print_json(out, &report)?,
                OutputFormat::Human => render::report_show(out, &id, &report)?,
            }
        }
        ReportCommand::Issue { id } => {
            let draft = issue_draft(&store.read(&id)?);
            match format {
                OutputFormat::Json => print_json(out, &draft)?,
                OutputFormat::Human => render::issue(out, &draft)?,
            }
        }
        ReportCommand::Delete { id, yes } => return delete(out, store, &id, yes, format),
        ReportCommand::Clear { yes } => return clear(out, store, yes, format),
        ReportCommand::Consent { choice: None, .. } => {
            show_consent(out, store.consent()?, format)?;
        }
        ReportCommand::Consent {
            choice: Some(choice),
            yes,
        } => return set_consent(out, store, choice.into(), yes, format),
    }
    Ok(ExitCode::SUCCESS)
}

/// `carrova report list`
fn list(out: &mut impl Write, store: &ReportStore, format: OutputFormat) -> Result<()> {
    let list = store.list()?;
    match format {
        OutputFormat::Json => print_json(out, &list),
        OutputFormat::Human => Ok(render::report_list(out, &list, store.consent()?)?),
    }
}

/// `carrova report delete ID`
fn delete(
    out: &mut impl Write,
    store: &ReportStore,
    id: &ReportId,
    yes: bool,
    format: OutputFormat,
) -> Result<ExitCode> {
    ensure_can_confirm(yes, "deleting a report")?;
    if !confirm(
        yes,
        &format!("Delete error report {id}? This cannot be undone."),
    )? {
        eprintln!("delete cancelled; nothing deleted");
        return Ok(ExitCode::FAILURE);
    }
    store.delete(id)?;
    match format {
        OutputFormat::Json => print_json(out, &json!({ "deleted": id }))?,
        OutputFormat::Human => writeln!(out, "Deleted error report {id}.")?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `carrova report clear`
fn clear(
    out: &mut impl Write,
    store: &ReportStore,
    yes: bool,
    format: OutputFormat,
) -> Result<ExitCode> {
    ensure_can_confirm(yes, "deleting all reports")?;
    if !confirm(yes, "Delete every error report? This cannot be undone.")? {
        eprintln!("clear cancelled; nothing deleted");
        return Ok(ExitCode::FAILURE);
    }
    store.clear()?;
    match format {
        OutputFormat::Json => print_json(out, &json!({ "cleared": true }))?,
        OutputFormat::Human => writeln!(out, "Deleted every error report.")?,
    }
    Ok(ExitCode::SUCCESS)
}

/// `carrova report consent allow|deny`. Denying deletes stored reports, so
/// it is confirmed first when there are any.
fn set_consent(
    out: &mut impl Write,
    store: &ReportStore,
    consent: ReportConsent,
    yes: bool,
    format: OutputFormat,
) -> Result<ExitCode> {
    let stored = store.list()?.len();
    if consent == ReportConsent::Denied && stored > 0 {
        ensure_can_confirm(yes, "turning error reports off")?;
        let question = format!(
            "Turn error reports off? The {stored} stored report(s) are deleted; \
             this cannot be undone."
        );
        if !confirm(yes, &question)? {
            eprintln!("consent unchanged; nothing deleted");
            return Ok(ExitCode::FAILURE);
        }
    }
    store.set_consent(consent)?;
    show_consent(out, consent, format)?;
    Ok(ExitCode::SUCCESS)
}

/// The consent choice as text or `{"consent": …}`.
fn show_consent(out: &mut impl Write, consent: ReportConsent, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => print_json(out, &json!({ "consent": consent })),
        OutputFormat::Human => Ok(render::consent(out, consent)?),
    }
}
