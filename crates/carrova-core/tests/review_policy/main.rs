//! The one resume policy every front-end shares
//! ([`Importer::plan_for_review`]): an approved plan of a resumed session
//! is reused without asking again, unless the user's options changed.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/archive.rs"]
mod archive_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use archive_support::{Side, archive, pass};
use carrova_core::{ConflictRule, Importer, PlanApproval, PlanOptions, State};

/// A receiver with an approved plan for `options`, reopened as after a
/// restart.
fn resumed_with_approved(options: &PlanOptions) -> (Side, Importer) {
    let r = Side::new();
    let mut imp = Importer::open(r.engine.clone(), &archive().path, &pass()).unwrap();
    assert_eq!(
        imp.plan_for_review(options).unwrap().1,
        PlanApproval::Needed
    );
    imp.approve().unwrap();
    let id = imp.id().clone();
    drop(imp);
    let imp = Importer::resume_archive(r.engine.clone(), &id, &pass()).unwrap();
    assert_eq!(imp.state(), State::Planned);
    assert!(imp.record().approved);
    (r, imp)
}

#[test]
fn a_new_plan_always_needs_approval() {
    let r = Side::new();
    let mut imp = Importer::open(r.engine.clone(), &archive().path, &pass()).unwrap();
    let approval = imp.plan_for_review(&PlanOptions::default()).unwrap().1;
    assert_eq!(approval, PlanApproval::Needed);
    assert_eq!(imp.state(), State::Planned);
    assert!(!imp.record().approved);
}

#[test]
fn an_approved_plan_is_reused_on_resume_with_the_same_options() {
    let options = PlanOptions::default();
    let (_r, mut imp) = resumed_with_approved(&options);
    let before = imp.current_plan().cloned();
    assert_eq!(
        imp.plan_for_review(&options).unwrap().1,
        PlanApproval::Given
    );
    assert!(imp.record().approved, "approval kept");
    assert_eq!(imp.current_plan().cloned(), before);
}

#[test]
fn changed_options_on_resume_plan_afresh_and_need_approval_again() {
    let (_r, mut imp) = resumed_with_approved(&PlanOptions::default());
    let changed = PlanOptions {
        default_rule: ConflictRule::KeepBoth,
        ..PlanOptions::default()
    };
    assert_eq!(
        imp.plan_for_review(&changed).unwrap().1,
        PlanApproval::Needed
    );
    assert!(!imp.record().approved, "approval reset");
    assert_eq!(imp.current_plan().unwrap().options, changed);
}

#[test]
fn a_new_session_has_no_plan_options_yet() {
    let r = Side::new();
    let imp = Importer::open(r.engine.clone(), &archive().path, &pass()).unwrap();
    assert_eq!(imp.plan_options(), None);
}

#[test]
fn a_resumed_session_reports_the_options_it_was_approved_with() {
    let approved = PlanOptions {
        default_rule: ConflictRule::KeepBoth,
        ..PlanOptions::default()
    };
    let (_r, mut imp) = resumed_with_approved(&approved);
    assert_eq!(imp.plan_options(), Some(&approved));

    // A front-end planning from them (no new user choices) keeps the approval.
    let stored = imp.plan_options().cloned().unwrap();
    assert_eq!(imp.plan_for_review(&stored).unwrap().1, PlanApproval::Given);
}

#[test]
fn accept_all_survives_resume() {
    let approved = PlanOptions {
        accept_safe_picks: true,
        ..PlanOptions::default()
    };
    let (_r, mut imp) = resumed_with_approved(&approved);
    assert!(imp.plan_options().is_some_and(|o| o.accept_safe_picks));
    let stored = imp.plan_options().cloned().unwrap();
    assert_eq!(imp.plan_for_review(&stored).unwrap().1, PlanApproval::Given);
}
