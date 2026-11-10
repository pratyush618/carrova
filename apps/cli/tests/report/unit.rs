use std::path::Path;

use carrova_platform::fake::FakePlatform;
use carrova_report::ReportConsent;

use super::*;

fn reports(dir: &Path) -> Reports {
    let platform = FakePlatform::with_home(dir.join("home/ana")).with_state_dir(dir.join("state"));
    Reports::from_platform(std::sync::Arc::new(platform)).unwrap()
}

#[test]
fn the_store_lives_under_the_platform_state_dir() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(
        reports(dir.path()).store.dir(),
        dir.path().join("state/reports")
    );
}

#[test]
fn recorded_errors_need_consent_and_are_redacted() {
    let dir = tempfile::tempdir().unwrap();
    let reports = reports(dir.path());
    let home = dir.path().join("home/ana");
    let error =
        anyhow::anyhow!("cannot open {}/.ssh/id_ed25519", home.display()).context("import failed");

    reports.record(&error);
    assert!(reports.store.list().unwrap().is_empty());

    reports.store.set_consent(ReportConsent::Allowed).unwrap();
    reports.record(&error);
    let list = reports.store.list().unwrap();
    let report = reports.store.read(&list[0].id).unwrap();
    assert_eq!(
        report.message_chain,
        ["import failed", "cannot open ~/<path>"]
    );
}
