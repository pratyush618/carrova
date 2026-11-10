use std::borrow::Cow;

use super::{error_report, human_size, safe};

#[test]
fn safe_escapes_terminal_controls() {
    assert_eq!(
        safe("home:.bashrc\x1b[1A\x1b[2K\rok\n\u{9b}x\u{202e}y\u{2066}\x7f"),
        "home:.bashrc\\u{1b}[1A\\u{1b}[2K\\u{d}ok\\u{a}\\u{9b}x\\u{202e}y\\u{2066}\\u{7f}"
    );
    assert!(matches!(safe("plain/ü 日本 \\ \""), Cow::Borrowed(_)));
    assert_eq!(safe("plain/ü 日本 \\ \""), "plain/ü 日本 \\ \"");
}

#[test]
fn error_report_dedupes_repeated_cause() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "No such file or directory");
    let err = anyhow::Error::new(Wrapped(io)).context("opening archive");
    let r = error_report(&err);
    assert_eq!(
        r,
        "opening archive\nCaused by:\n    I/O error on /x: No such file or directory"
    );
}

#[test]
fn error_report_keeps_distinct_causes_and_escapes() {
    let err = anyhow::anyhow!("bad \x1b[2K item").context("import failed");
    let r = error_report(&err);
    assert_eq!(r, "import failed\nCaused by:\n    bad \\u{1b}[2K item");
}

#[derive(Debug)]
struct Wrapped(std::io::Error);

impl std::fmt::Display for Wrapped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "I/O error on /x: {}", self.0)
    }
}

impl std::error::Error for Wrapped {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[test]
fn human_size_picks_si_unit() {
    assert_eq!(human_size(0), "0 B");
    assert_eq!(human_size(999), "999 B");
    assert_eq!(human_size(1500), "1.5 kB");
    assert_eq!(human_size(5_000_000), "5.0 MB");
    assert_eq!(human_size(999_999), "1.0 MB", "never `1000.0 kB`");
    assert_eq!(human_size(u64::MAX), "18.4 EB");
}
