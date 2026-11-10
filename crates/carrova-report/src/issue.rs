//! "Send" = a prefilled GitHub new-issue URL the user opens in their
//! browser after reading the exact text. Carrova itself sends nothing.

use std::fmt::Write as _;

use serde::Serialize;

use crate::report::{ErrorReport, ReportKind};

/// `pratyush618/carrova` of the issue tracker. **Placeholder**: replace before the
/// first public release (see `packaging/README.md`).
pub const ISSUE_REPOSITORY: &str = "pratyush618/carrova";
/// Longest percent-encoded body put in the URL. Keeps the whole URL well
/// under the ~8 KiB browsers and GitHub accept.
pub const MAX_ENCODED_BODY: usize = 6000;
/// Appended when the body had to be cut.
const TRUNCATION_NOTE: &str = "\n\n_(report truncated)_";
/// Longest issue title, in characters.
const MAX_TITLE_CHARS: usize = 120;

/// What the user reviews before sending, and the URL that sends it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct IssueDraft {
    /// Issue title.
    pub title: String,
    /// Issue body (Markdown), exactly as encoded in [`Self::url`].
    pub body: String,
    /// `https://github.com/pratyush618/carrova/issues/new?title=…&body=…`.
    pub url: String,
}

/// Builds the issue for `report`; the body is truncated so its encoded
/// form fits [`MAX_ENCODED_BODY`].
#[must_use]
pub fn issue_draft(report: &ErrorReport) -> IssueDraft {
    let title: String = format!("{}: {}", kind_label(report.kind), report.headline())
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect();
    let body = fit_encoded(&issue_body(report), MAX_ENCODED_BODY);
    let url = format!(
        "https://github.com/{ISSUE_REPOSITORY}/issues/new?title={}&body={}",
        percent_encode(&title),
        percent_encode(&body),
    );
    IssueDraft { title, body, url }
}

fn kind_label(kind: ReportKind) -> &'static str {
    match kind {
        ReportKind::Panic => "Crash",
        ReportKind::Error => "Error",
    }
}

fn issue_body(report: &ErrorReport) -> String {
    let mut body = format!(
        "**Carrova** {} on {} ({})\n\n### Messages\n",
        report.app_version,
        report.os,
        kind_label(report.kind)
    );
    for message in &report.message_chain {
        // `fmt::Write` for `String` never fails.
        let _ = writeln!(body, "- {message}");
    }
    if !report.backtrace_frames.is_empty() {
        body.push_str("\n### Backtrace (Carrova frames)\n```\n");
        for frame in &report.backtrace_frames {
            let _ = writeln!(body, "{frame}");
        }
        body.push_str("```\n");
    }
    body
}

/// `body`, cut at a char boundary (plus a note) so its encoding fits `limit`.
fn fit_encoded(body: &str, limit: usize) -> String {
    if percent_encode(body).len() <= limit {
        return body.to_owned();
    }
    let budget = limit.saturating_sub(percent_encode(TRUNCATION_NOTE).len());
    let mut used = 0;
    let mut cut = String::new();
    for ch in body.chars() {
        used += encoded_len(ch);
        if used > budget {
            break;
        }
        cut.push(ch);
    }
    cut.push_str(TRUNCATION_NOTE);
    cut
}

/// RFC 3986 percent-encoding: unreserved characters pass, every other
/// UTF-8 byte becomes `%XX`.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if is_unreserved(byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

fn encoded_len(ch: char) -> usize {
    let mut buf = [0; 4];
    ch.encode_utf8(&mut buf)
        .bytes()
        .map(|byte| if is_unreserved(byte) { 1 } else { 3 })
        .sum()
}

const fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

#[cfg(test)]
#[path = "../tests/issue/unit.rs"]
mod tests;
