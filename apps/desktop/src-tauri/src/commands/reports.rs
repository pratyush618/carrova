//! Error-report commands: thin IPC over the [`Reports`] store.
//!
//! The UI shows a report's exact issue text before opening its URL with
//! the opener plugin. Commands are `async` so their file I/O runs on
//! Tauri's async pool, not the main (UI) thread.

use carrova_report::{ErrorReport, IssueDraft, ReportConsent, ReportId, ReportSummary};
use tauri::State;

use crate::command_error::CommandError;
use crate::error_report::Reports;

/// `CarrovaPort.errorReports`: stored reports, newest first.
#[tauri::command]
pub async fn error_reports(
    reports: State<'_, Reports>,
) -> Result<Vec<ReportSummary>, CommandError> {
    Ok(reports.store().list()?)
}

/// `CarrovaPort.errorReport`: one full report.
#[tauri::command]
pub async fn error_report(
    reports: State<'_, Reports>,
    id: ReportId,
) -> Result<ErrorReport, CommandError> {
    Ok(reports.store().read(&id)?)
}

/// `CarrovaPort.deleteErrorReport`: deletes one report.
#[tauri::command]
pub async fn delete_error_report(
    reports: State<'_, Reports>,
    id: ReportId,
) -> Result<(), CommandError> {
    Ok(reports.store().delete(&id)?)
}

/// `CarrovaPort.clearErrorReports`: deletes every report.
#[tauri::command]
pub async fn clear_error_reports(reports: State<'_, Reports>) -> Result<(), CommandError> {
    Ok(reports.store().clear()?)
}

/// `CarrovaPort.errorReportIssue`: the GitHub issue (title, body, URL) for
/// one report, for the user to review and open.
#[tauri::command]
pub async fn error_report_issue(
    reports: State<'_, Reports>,
    id: ReportId,
) -> Result<IssueDraft, CommandError> {
    Ok(carrova_report::issue_draft(&reports.store().read(&id)?))
}

/// `CarrovaPort.errorReportConsent`: the persisted consent choice.
#[tauri::command]
pub async fn error_report_consent(
    reports: State<'_, Reports>,
) -> Result<ReportConsent, CommandError> {
    Ok(reports.store().consent()?)
}

/// `CarrovaPort.setErrorReportConsent`: persists the consent choice
/// (denying also deletes stored reports).
#[tauri::command]
pub async fn set_error_report_consent(
    reports: State<'_, Reports>,
    consent: ReportConsent,
) -> Result<(), CommandError> {
    Ok(reports.store().set_consent(consent)?)
}
