//! The report itself and how it is built from an error or a panic.

use std::backtrace::Backtrace;
use std::error::Error;
use std::panic::PanicHookInfo;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::frames::our_frames;
use crate::redact::Redactor;

/// What produced a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    /// The process panicked.
    Panic,
    /// An operation failed with an error the user saw.
    Error,
}

/// A redacted crash/error report. Holds no file contents, paths, secrets
/// or pairing codes: every text field went through [`Redactor`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ErrorReport {
    /// Carrova version that produced the report.
    pub app_version: String,
    /// Operating system family (`carrova_platform::OsKind` display name).
    pub os: String,
    /// Creation time, Unix seconds.
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub when: u64,
    /// Panic or error.
    pub kind: ReportKind,
    /// Redacted messages, outermost first (error `source()` chain, or the
    /// panic message and location).
    pub message_chain: Vec<String>,
    /// Symbol names of Carrova's own frames (empty for errors, whose
    /// backtrace would only show the reporting site).
    pub backtrace_frames: Vec<String>,
}

/// Facts every report carries, plus the redaction policy for this user.
/// Built once per process by the front-end.
#[derive(Debug, Clone)]
pub struct ReportContext {
    app_version: String,
    os: String,
    redactor: Redactor,
}

impl ReportContext {
    /// `os` comes from `carrova_platform::OsKind`'s `Display`; `home` from
    /// `Platform::resolve_root(SemanticRoot::Home)` (`None` if unknown).
    #[must_use]
    pub fn new(app_version: impl Into<String>, os: impl Into<String>, home: Option<&Path>) -> Self {
        Self {
            app_version: app_version.into(),
            os: os.into(),
            redactor: Redactor::new(home),
        }
    }

    /// The redaction policy for this user.
    #[must_use]
    pub fn redactor(&self) -> &Redactor {
        &self.redactor
    }

    fn report(&self, kind: ReportKind, messages: &[String], frames: Vec<String>) -> ErrorReport {
        ErrorReport {
            app_version: self.app_version.clone(),
            os: self.os.clone(),
            when: crate::clock::since_epoch().as_secs(),
            kind,
            message_chain: messages
                .iter()
                .map(|message| self.redactor.redact(message))
                .collect(),
            backtrace_frames: frames,
        }
    }
}

impl ErrorReport {
    /// A report of `error` and its `source()` chain.
    #[must_use]
    pub fn from_error(error: &(dyn Error + 'static), context: &ReportContext) -> Self {
        let chain: Vec<String> = std::iter::successors(Some(error), |&err| err.source())
            .map(ToString::to_string)
            .collect();
        context.report(ReportKind::Error, &chain, Vec::new())
    }

    /// A report of a panic, with the Carrova frames of `backtrace`.
    #[must_use]
    pub fn from_panic(
        info: &PanicHookInfo<'_>,
        backtrace: &Backtrace,
        context: &ReportContext,
    ) -> Self {
        let mut messages = vec![panic_message(info)];
        if let Some(location) = info.location() {
            messages.push(format!("at {}:{}", location.file(), location.line()));
        }
        context.report(
            ReportKind::Panic,
            &messages,
            our_frames(&backtrace.to_string()),
        )
    }

    /// First message of the chain (the one-line summary).
    #[must_use]
    pub fn headline(&self) -> &str {
        self.message_chain.first().map_or("", String::as_str)
    }
}

fn panic_message(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic with a non-text payload".to_owned())
}
