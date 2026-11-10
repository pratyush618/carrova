//! Opt-in, redacted local crash and error reports.
//!
//! Shared by every front-end so the CLI and the desktop app behave
//! identically:
//!
//! - [`ReportContext`] holds the app version, OS name and the
//!   [`Redactor`] for this user (home directory passed in by the caller).
//! - [`ErrorReport::from_error`] / [`install_panic_hook`] build reports;
//!   every message goes through the redaction policy first.
//! - [`ReportStore`] keeps them as JSON under Carrova's state directory
//!   (newest [`MAX_REPORTS`]) together with the user's [`ReportConsent`],
//!   every file written private to the user through [`ReportFs`].
//!   [`ReportStore::save`] is the single consent gate: without an
//!   explicit `Allowed`, nothing is ever written.
//! - [`issue_draft`] turns a report into a prefilled GitHub issue the
//!   user reviews and opens themselves. Carrova makes no network
//!   calls for reporting.
//!
//! This crate is OS-neutral: it never looks up paths or user names itself,
//! and files are made private (`0600`) through the caller's [`ReportFs`]
//! (front-ends implement it over their `Platform`).

mod clock;
mod consent;
mod error;
mod frames;
mod fs;
mod hook;
mod issue;
mod redact;
mod report;
mod store;

pub use consent::ReportConsent;
pub use error::ReportError;
pub use fs::ReportFs;
pub use hook::install_panic_hook;
pub use issue::{ISSUE_REPOSITORY, IssueDraft, MAX_ENCODED_BODY, issue_draft};
pub use redact::Redactor;
pub use report::{ErrorReport, ReportContext, ReportKind};
pub use store::{MAX_REPORTS, REPORTS_DIR, ReportId, ReportStore, ReportSummary};
