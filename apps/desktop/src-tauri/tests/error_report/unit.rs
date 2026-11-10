use std::io;
use std::sync::Arc;

use carrova_platform::fake::FakePlatform;
use carrova_report::ReportConsent;

use super::*;

fn reports(dir: &std::path::Path) -> Reports {
    let platform = FakePlatform::with_home(dir.join("home/ana")).with_state_dir(dir.join("state"));
    Reports::from_platform(Arc::new(platform)).unwrap()
}

#[test]
fn store_lives_under_the_platform_state_dir() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        reports(dir.path()).store().dir(),
        dir.path().join("state/reports")
    );
}

#[test]
fn recorded_errors_are_redacted_and_need_consent() {
    let dir = tempfile::tempdir().unwrap();
    let reports = reports(dir.path());
    let home = dir.path().join("home/ana");
    let error = io::Error::other(format!("cannot open {}/.ssh/id_ed25519", home.display()));

    assert_eq!(reports.record(&error).unwrap(), None);

    reports.store().set_consent(ReportConsent::Allowed).unwrap();
    let id = reports.record(&error).unwrap().unwrap();
    let report = reports.store().read(&id).unwrap();
    assert_eq!(report.message_chain, ["cannot open ~/<path>"]);
    assert_eq!(report.app_version, APP_VERSION);
}

#[test]
fn a_failure_is_recorded_only_with_consent_and_never_fails_the_caller() {
    let dir = tempfile::tempdir().unwrap();
    let reports = reports(dir.path());
    let failure = io::Error::other("the session stopped unexpectedly");

    reports.record_failure(&failure);
    assert!(reports.store().list().unwrap().is_empty());

    reports.store().set_consent(ReportConsent::Allowed).unwrap();
    reports.record_failure(&failure);
    let list = reports.store().list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].headline, "the session stopped unexpectedly");
}

#[test]
#[cfg(unix)] // Windows files have no mode bits (profile ACLs keep them private)
fn reports_and_the_consent_choice_are_private_to_the_user() {
    use carrova_platform::Platform;

    let dir = tempfile::tempdir().unwrap();
    let platform = Arc::new(
        FakePlatform::with_home(dir.path().join("home/ana"))
            .with_state_dir(dir.path().join("state")),
    );
    let reports = Reports::from_platform(platform.clone()).unwrap();
    reports.store().set_consent(ReportConsent::Allowed).unwrap();
    let id = reports.record(&io::Error::other("boom")).unwrap().unwrap();

    for file in ["consent.json".to_owned(), format!("{id}.json")] {
        let meta = platform
            .metadata(&reports.store().dir().join(&file))
            .unwrap();
        assert_eq!(meta.mode, Some(0o600), "{file}");
    }
}
