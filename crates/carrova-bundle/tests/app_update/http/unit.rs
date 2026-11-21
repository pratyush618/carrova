use super::*;

#[test]
fn content_ranges_read_start_and_total() {
    assert_eq!(
        content_range(Some("bytes 100-199/200")),
        Some((100, Some(200)))
    );
    assert_eq!(content_range(Some("bytes 100-199/*")), Some((100, None)));
    assert_eq!(content_range(Some("items 1-2/3")), None);
    assert_eq!(content_range(None), None);
}

#[test]
fn ureq_failures_keep_only_their_kind() {
    let io = ureq::Error::Io(io::Error::new(
        io::ErrorKind::ConnectionReset,
        "https://example.com/?token=secret",
    ));
    let failure = net_failure(&io);
    assert_eq!(failure, NetFailure::Read(io::ErrorKind::ConnectionReset));
    assert!(!failure.to_string().contains("token"));
    assert_eq!(
        net_failure(&ureq::Error::ConnectionFailed),
        NetFailure::Connect
    );
}
