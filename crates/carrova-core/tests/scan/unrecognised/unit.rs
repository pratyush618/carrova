use super::*;

#[test]
fn measuring_stops_once_cancelled() {
    let dir = tempfile::TempDir::new().unwrap();
    let cancel = Cancel::default();
    cancel.cancel();

    let measured = measure(dir.path(), &cancel);

    assert!(matches!(measured, Err(ScanError::Cancelled)));
}

#[test]
fn measuring_counts_regular_files() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("a"), b"12345").unwrap();

    let measured = measure(dir.path(), &Cancel::default());

    assert!(matches!(measured, Ok((5, 1, 0))));
}
