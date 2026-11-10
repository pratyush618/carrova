//! Panic hook that records a redacted report, then defers to the
//! previously installed hook (so the usual message still prints).

use std::backtrace::Backtrace;
use std::panic;

use crate::consent::ReportConsent;
use crate::report::{ErrorReport, ReportContext};
use crate::store::ReportStore;

/// Installs the reporting panic hook, chained to the hook it replaces.
/// Call once at start-up.
///
/// The consent is checked when a panic happens ([`ReportStore::save`]
/// reads the persisted choice), so turning reporting on or off takes
/// effect immediately. Without `Allowed`, the hook only runs the previous
/// one and writes nothing.
pub fn install_panic_hook(store: ReportStore, context: ReportContext) {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        previous(info);
        // Checked here too only to skip the backtrace capture; `save` is the gate.
        if !store.consent().is_ok_and(ReportConsent::is_allowed) {
            return;
        }
        let report = ErrorReport::from_panic(info, &Backtrace::force_capture(), &context);
        if let Err(err) = store.save(&report) {
            eprintln!("carrova: could not save the crash report: {err}");
        }
    }));
}
