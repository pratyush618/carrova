//! The panic hook is process-global, so its behaviour is checked in one
//! test of its own binary.

#[path = "../support/mod.rs"]
mod support;

use std::panic;
use std::path::Path;

use carrova_report::{ReportConsent, ReportContext, ReportKind, install_panic_hook};

#[test]
fn hook_records_a_redacted_panic_only_while_consent_is_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let (store, fs) = support::store_in(dir.path());
    let context = ReportContext::new("0.0.1", "Linux", Some(Path::new("/home/ana")));
    install_panic_hook(store.clone(), context);

    let _ = panic::catch_unwind(|| panic!("before consent"));
    store.set_consent(ReportConsent::Denied).unwrap();
    let _ = panic::catch_unwind(|| panic!("consent denied"));
    assert!(store.list().unwrap().is_empty());

    store.set_consent(ReportConsent::Allowed).unwrap();
    let _ = panic::catch_unwind(|| {
        panic!("cannot read /home/ana/.ssh/id_ed25519 for code 7-orbit-lemon")
    });
    let _ = panic::take_hook();

    let reports = store.list().unwrap();
    assert_eq!(reports.len(), 1);
    let report = store.read(&reports[0].id).unwrap();
    assert_eq!(report.kind, ReportKind::Panic);
    assert!(
        fs.private_files()
            .contains(&store.dir().join(format!("{}.json", reports[0].id)))
    );
    assert_eq!(
        report.message_chain[0],
        "cannot read ~/<path> for code <pairing-code>"
    );
    // `file!()`: the location as the compiler spells it on this OS.
    assert!(
        report.message_chain[1].starts_with(&format!("at {}:", file!())),
        "{}",
        report.message_chain[1]
    );
    assert!(
        report
            .backtrace_frames
            .iter()
            .all(|frame| frame.contains("carrova_"))
    );
}
