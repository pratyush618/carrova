use super::*;

#[test]
fn broadcast_reply_parses_port_only_with_prefix() {
    let mut r = BROADCAST_REPLY.to_vec();
    r.extend_from_slice(b"40123");
    assert_eq!(parse_reply(&r), Some((40123, None)));
    assert_eq!(parse_reply(b"40123"), None);
    let mut bad = BROADCAST_REPLY.to_vec();
    bad.extend_from_slice(b"70000");
    assert_eq!(parse_reply(&bad), None);
}

#[test]
fn browse_reply_carries_the_device_name_after_the_port() {
    let named = [BROADCAST_REPLY, b"40123 ezio's laptop"].concat();
    assert_eq!(
        parse_reply(&named),
        Some((40123, Some("ezio's laptop".to_owned())))
    );
    let not_utf8 = [BROADCAST_REPLY, b"40123 \xff"].concat();
    assert_eq!(parse_reply(&not_utf8), None);
}
