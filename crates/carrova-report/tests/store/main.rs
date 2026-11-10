//! `ReportStore`: consent-gated saving, listing, pruning, deletion and
//! path-safe report ids.

#![allow(clippy::unwrap_used)]

#[path = "../support/mod.rs"]
mod support;

use std::fs;

use carrova_report::{
    ErrorReport, MAX_REPORTS, ReportConsent, ReportError, ReportId, ReportKind, ReportStore,
    ReportSummary,
};
use support::store_in;

fn report(headline: &str) -> ErrorReport {
    ErrorReport {
        app_version: "0.0.1".into(),
        os: "Linux".into(),
        when: 7,
        kind: ReportKind::Error,
        message_chain: vec![headline.into()],
        backtrace_frames: Vec::new(),
    }
}

fn store() -> (tempfile::TempDir, ReportStore) {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_in(dir.path());
    store.set_consent(ReportConsent::Allowed).unwrap();
    (dir, store)
}

#[test]
fn nothing_is_written_without_explicit_consent() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_in(dir.path());
    assert_eq!(store.save(&report("unset")).unwrap(), None);
    assert!(!store.dir().exists());

    store.set_consent(ReportConsent::Denied).unwrap();
    assert_eq!(store.save(&report("denied")).unwrap(), None);
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn saved_report_reads_back_and_is_listed() {
    let (_dir, store) = store();
    let id = store.save(&report("boom")).unwrap().unwrap();

    assert_eq!(store.read(&id).unwrap(), report("boom"));
    assert_eq!(
        store.list().unwrap(),
        [ReportSummary {
            id,
            when: 7,
            kind: ReportKind::Error,
            headline: "boom".into()
        }]
    );
}

#[test]
fn only_the_newest_reports_are_kept() {
    let (_dir, store) = store();
    let ids: Vec<ReportId> = (0..MAX_REPORTS + 3)
        .map(|n| store.save(&report(&n.to_string())).unwrap().unwrap())
        .collect();

    let listed: Vec<ReportId> = store.list().unwrap().into_iter().map(|s| s.id).collect();
    let newest: Vec<ReportId> = ids.iter().rev().take(MAX_REPORTS).cloned().collect();
    assert_eq!(listed, newest);
}

#[test]
fn delete_and_clear_remove_reports() {
    let (_dir, store) = store();
    let first = store.save(&report("a")).unwrap().unwrap();
    store.save(&report("b")).unwrap();

    store.delete(&first).unwrap();
    assert!(matches!(store.read(&first), Err(ReportError::NotFound(_))));
    assert!(matches!(
        store.delete(&first),
        Err(ReportError::NotFound(_))
    ));
    store.clear().unwrap();
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn listing_skips_foreign_and_malformed_files() {
    let (_dir, store) = store();
    store.save(&report("ok")).unwrap();
    fs::write(store.dir().join("notes.json"), "{}").unwrap();
    fs::write(store.dir().join("00000000000000000001.json"), "not json").unwrap();

    let headlines: Vec<String> = store
        .list()
        .unwrap()
        .into_iter()
        .map(|s| s.headline)
        .collect();
    assert_eq!(headlines, ["ok"]);
}

#[test]
fn ids_that_could_name_other_paths_are_rejected() {
    for bad in [
        "../consent",
        "1234",
        "0000000000000000000a",
        "/etc/passwd",
        "",
    ] {
        assert!(
            matches!(bad.parse::<ReportId>(), Err(ReportError::InvalidId(_))),
            "{bad}"
        );
    }
    let json = serde_json::to_string("../../x").unwrap();
    assert!(serde_json::from_str::<ReportId>(&json).is_err());
}

#[test]
fn consent_defaults_to_unset_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_in(dir.path());
    assert_eq!(store.consent().unwrap(), ReportConsent::Unset);

    store.set_consent(ReportConsent::Allowed).unwrap();
    assert_eq!(
        store_in(dir.path()).0.consent().unwrap(),
        ReportConsent::Allowed
    );
}

#[test]
fn denying_consent_deletes_stored_reports() {
    let (_dir, store) = store();
    store.save(&report("a")).unwrap();

    store.set_consent(ReportConsent::Denied).unwrap();

    assert!(store.list().unwrap().is_empty());
    assert_eq!(store.consent().unwrap(), ReportConsent::Denied);
}

#[test]
fn reports_and_consent_are_written_as_private_files_and_moved_into_place() {
    let dir = tempfile::tempdir().unwrap();
    let (store, fs) = store_in(dir.path());
    store.set_consent(ReportConsent::Allowed).unwrap();
    let id = store.save(&report("private")).unwrap().unwrap();

    assert_eq!(
        fs.private_files(),
        [
            store.dir().join("consent.json"),
            store.dir().join(format!("{id}.json")),
        ]
    );
    let leftovers: Vec<_> = fs::read_dir(store.dir())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|name| name.to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn consent_can_be_changed_again_after_an_interrupted_write() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _) = store_in(dir.path());
    fs::create_dir_all(store.dir()).unwrap();
    fs::write(store.dir().join(".consent.json.tmp"), "half").unwrap();

    store.set_consent(ReportConsent::Allowed).unwrap();
    store.set_consent(ReportConsent::Denied).unwrap();
    assert_eq!(store.consent().unwrap(), ReportConsent::Denied);
}
