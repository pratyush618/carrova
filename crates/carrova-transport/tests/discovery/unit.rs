use super::*;

#[test]
fn broadcast_reply_parses_port_only_with_prefix() {
    let mut r = BROADCAST_REPLY.to_vec();
    r.extend_from_slice(b"40123");
    assert_eq!(parse_reply(&r), Some(40123));
    assert_eq!(parse_reply(b"40123"), None);
    let mut bad = BROADCAST_REPLY.to_vec();
    bad.extend_from_slice(b"70000");
    assert_eq!(parse_reply(&bad), None);
}
