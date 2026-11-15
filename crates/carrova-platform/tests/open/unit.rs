use super::*;
use std::path::Path;

#[test]
fn only_web_urls_open() {
    assert!(checked_url("https://example.org/a?b=c").is_ok());
    assert!(checked_url("HTTP://example.org").is_ok());
    for bad in [
        "file:///etc/passwd",
        "ssh://host",
        "-o",
        "javascript:1",
        "é",
    ] {
        assert!(
            matches!(checked_url(bad), Err(PlatformError::InvalidInput(_))),
            "{bad}"
        );
    }
}

#[test]
fn only_absolute_paths_open() {
    // `/home/a` is not absolute on Windows (no drive); the temp dir is everywhere.
    assert!(checked_path(&std::env::temp_dir().join("doc.pdf")).is_ok());
    assert!(checked_path(Path::new("-rf")).is_err());
    assert!(checked_path(Path::new("doc.pdf")).is_err());
}
