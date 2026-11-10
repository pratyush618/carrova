//! `ErrorReport::from_error`: the source chain is captured outermost first
//! and redacted.

use std::error::Error;
use std::fmt;
use std::path::Path;

use carrova_report::{ErrorReport, ReportContext, ReportKind};

#[derive(Debug)]
struct Failure {
    message: &'static str,
    source: Option<Box<Failure>>,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

#[test]
fn error_report_carries_the_redacted_source_chain_outermost_first() {
    let error = Failure {
        message: "import failed",
        source: Some(Box::new(Failure {
            message: "cannot open /home/ana/.ssh/id_ed25519",
            source: None,
        })),
    };
    let context = ReportContext::new("0.0.1", "Linux", Some(Path::new("/home/ana")));

    let report = ErrorReport::from_error(&error, &context);

    assert_eq!(report.kind, ReportKind::Error);
    assert_eq!(report.app_version, "0.0.1");
    assert_eq!(report.os, "Linux");
    assert_eq!(
        report.message_chain,
        ["import failed", "cannot open ~/<path>"]
    );
    assert_eq!(report.headline(), "import failed");
    assert!(report.backtrace_frames.is_empty());
    assert!(report.when > 0);
}
