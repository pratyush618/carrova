//! `sessions::rollback_preview`: what an undo would do, computed without
//! changing anything, and matching what the undo then does.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/apply.rs"]
mod apply_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/archive.rs"]
mod archive_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use apply_support::snapshot;
use archive_support::{Side, applied};
use carrova_core::{RollbackPreview, sessions};

#[test]
fn preview_counts_what_the_undo_then_does_and_changes_nothing() {
    let (r, imp) = applied();
    let id = imp.id().clone();
    let home_before = snapshot(&*r.platform, &r.home);
    let session_before = snapshot(&*r.platform, &r.session_dir(&id));

    let preview = sessions::rollback_preview(&r.engine, Some(&id)).unwrap();

    assert_eq!(snapshot(&*r.platform, &r.home), home_before);
    assert_eq!(snapshot(&*r.platform, &r.session_dir(&id)), session_before);
    assert!(preview.restored >= 1, "{preview:?}");
    assert!(preview.removed >= 1, "{preview:?}");
    assert!(preview.blockers.is_empty(), "{preview:?}");

    let report = sessions::rollback(&r.engine, Some(&id)).unwrap();
    assert_eq!(RollbackPreview::from(report), preview);
}

#[test]
fn a_file_changed_since_the_import_is_previewed_as_a_blocker() {
    let (r, imp) = applied();
    let id = imp.id().clone();
    r.put(".config/app/new.txt", b"edited by the user since");
    let home_before = snapshot(&*r.platform, &r.home);

    let preview = sessions::rollback_preview(&r.engine, Some(&id)).unwrap();

    assert_eq!(snapshot(&*r.platform, &r.home), home_before);
    assert_eq!(preview.blockers.len(), 1, "{preview:?}");
    assert!(preview.blockers[0].target.ends_with("new.txt"));
}

#[test]
fn preview_without_an_applied_import_is_refused() {
    let r = Side::new();
    assert!(sessions::rollback_preview(&r.engine, None).is_err());
}
