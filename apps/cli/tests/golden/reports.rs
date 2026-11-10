//! `carrova report list | show | issue | consent` text.

use carrova_report::{
    ErrorReport, ReportConsent, ReportId, ReportKind, ReportSummary, issue_draft,
};

use super::{ESC, golden, text};
use crate::render::report::{consent, issue, report_list, report_show};

fn id(n: u64) -> ReportId {
    format!("{n:020}").parse().unwrap()
}

/// 2026-11-08 21:44:55 UTC.
const WHEN: u64 = 1_794_174_295;

fn panic_report() -> ErrorReport {
    ErrorReport {
        app_version: "0.4.0".into(),
        os: "Linux".into(),
        when: WHEN,
        kind: ReportKind::Panic,
        message_chain: vec![
            format!("index out of bounds{ESC}"),
            "at crates/carrova-core/src/plan/mod.rs:42".into(),
        ],
        backtrace_frames: vec![
            "carrova_core::plan::compute".into(),
            "carrova_cli::session::import".into(),
        ],
    }
}

fn error_report() -> ErrorReport {
    ErrorReport {
        app_version: "0.4.0".into(),
        os: "Linux".into(),
        when: WHEN + 60,
        kind: ReportKind::Error,
        message_chain: vec![
            "cannot open ~/secret-plans.carrova".into(),
            "No such file or directory (os error 2)".into(),
        ],
        backtrace_frames: Vec::new(),
    }
}

fn summary(n: u64, r: &ErrorReport) -> ReportSummary {
    ReportSummary {
        id: id(n),
        when: r.when,
        kind: r.kind,
        headline: r.headline().to_owned(),
    }
}

#[test]
fn golden_report_list() {
    let list = [summary(2, &error_report()), summary(1, &panic_report())];
    let mut s = text(|o| report_list(o, &list, ReportConsent::Allowed));
    for choice in [ReportConsent::Denied, ReportConsent::Unset] {
        s.push_str("--\n");
        s.push_str(&text(|o| report_list(o, &[], choice)));
    }
    golden("report_list", &s);
}

#[test]
fn golden_report_show() {
    let mut s = text(|o| report_show(o, &id(1), &panic_report()));
    s.push_str("--\n");
    s.push_str(&text(|o| report_show(o, &id(2), &error_report())));
    golden("report_show", &s);
}

#[test]
fn golden_report_issue_and_consent() {
    let mut s = text(|o| issue(o, &issue_draft(&error_report())));
    for choice in [
        ReportConsent::Allowed,
        ReportConsent::Denied,
        ReportConsent::Unset,
    ] {
        s.push_str("--\n");
        s.push_str(&text(|o| consent(o, choice)));
    }
    golden("report_issue_consent", &s);
}
