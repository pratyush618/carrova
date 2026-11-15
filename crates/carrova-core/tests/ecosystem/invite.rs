//! The `carrova://pair` invite.

#![allow(clippy::unwrap_used)]

use carrova_core::CoreError;
use carrova_core::ecosystem::PairInvite;
use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::PairingCode;
use carrova_transport::Ticket;

fn invite(addr: Option<&str>) -> PairInvite {
    PairInvite {
        ticket: Ticket {
            code: PairingCode::generate().unwrap(),
            addr: addr.map(|a| a.parse().unwrap()),
        },
        key: DeviceKey::generate().unwrap().public(),
    }
}

#[test]
fn invite_uri_roundtrip() {
    for addr in [None, Some("192.168.1.20:4433")] {
        let i = invite(addr);
        let uri = i.to_uri();
        assert!(uri.starts_with("carrova://pair?v=1&t="), "{uri}");
        assert!(!uri.contains('@'), "ticket is percent-encoded: {uri}");
        let key = uri.rsplit("&k=").next().unwrap();
        assert_eq!(key.len(), 52);
        assert_eq!(uri.parse::<PairInvite>().unwrap(), i);
    }
}

#[test]
fn invite_rejects_unknown_version() {
    let uri = invite(None).to_uri().replace("v=1", "v=2");
    assert!(matches!(
        uri.parse::<PairInvite>(),
        Err(CoreError::InvalidInvite(_))
    ));
}

#[test]
fn invite_rejects_malformed() {
    let good = invite(None).to_uri();
    let (head, key) = good.split_once("&k=").unwrap();
    for bad in [
        "https://pair?v=1".to_owned(),
        "carrova://pair?v=1".to_owned(),
        head.to_owned(),
        format!("{head}&k={}", &key[1..]),
        format!("{head}&k={}", key.to_lowercase().replace('a', "1")),
        format!("{good}&x=1"),
    ] {
        assert!(
            matches!(bad.parse::<PairInvite>(), Err(CoreError::InvalidInvite(_))),
            "{bad}"
        );
    }
}
