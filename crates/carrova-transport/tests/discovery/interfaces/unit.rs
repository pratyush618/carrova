use super::*;

#[test]
fn implausible_discovery_answers_are_dropped() {
    let at = |ip: [u8; 4], port| SocketAddrV4::new(Ipv4Addr::from(ip), port);
    assert!(plausible_sender(at([192, 168, 1, 5], 40000), false));
    assert!(!plausible_sender(at([192, 168, 1, 5], 22), false));
    assert!(!plausible_sender(at([192, 168, 1, 5], 1023), false));
    assert!(plausible_sender(at([127, 0, 0, 1], 40000), true));
    assert!(!plausible_sender(at([127, 0, 0, 1], 40000), false));
    assert!(!plausible_sender(at([0, 0, 0, 0], 40000), true));
    assert!(!plausible_sender(at([255, 255, 255, 255], 40000), true));
    assert!(!plausible_sender(at([224, 0, 0, 251], 40000), true));
}

#[test]
fn primary_address_is_never_loopback() {
    if let Some(ip) = primary_lan_ipv4(carrova_platform::current().as_ref()) {
        assert!(!ip.is_loopback());
    }
}
