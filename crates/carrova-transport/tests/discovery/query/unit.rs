use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::KnownSecret;
use carrova_platform::OsKind;

use super::*;
use crate::discovery::presence::KnownTags;

fn offer(advert: PresenceAdvert) -> Announced {
    Announced {
        advert,
        offers_code: true,
    }
}

fn presence(advert: PresenceAdvert) -> Announced {
    Announced {
        advert,
        offers_code: false,
    }
}

fn named(name: &str) -> PresenceAdvert {
    PresenceAdvert {
        name: Some(name.to_owned()),
        ..PresenceAdvert::default()
    }
}

#[test]
fn broadcast_reply_parses_port_only_with_prefix() {
    let mut r = BROADCAST_REPLY.to_vec();
    r.extend_from_slice(b"40123");
    assert_eq!(
        parse_reply(&r),
        Some((40123, offer(PresenceAdvert::default())))
    );
    assert_eq!(parse_reply(b"40123"), None);
    let mut bad = BROADCAST_REPLY.to_vec();
    bad.extend_from_slice(b"70000");
    assert_eq!(parse_reply(&bad), None);
}

#[test]
fn browse_reply_carries_the_device_name_after_the_port() {
    let reply = [BROADCAST_REPLY, b"40123 ezio's laptop"].concat();
    assert_eq!(
        parse_reply(&reply),
        Some((40123, offer(named("ezio's laptop"))))
    );
    let not_utf8 = [BROADCAST_REPLY, b"40123 \xff"].concat();
    assert_eq!(parse_reply(&not_utf8), None);
}

#[test]
fn presence_reply_carries_its_txt_fields_one_per_line() {
    let known = KnownTags::fresh(
        &[KnownSecret::from_bytes([1; 32])],
        &DeviceKey::from_bytes(&[2; 32]).public(),
    )
    .unwrap()
    .unwrap();
    let body = format!(
        "47821\nkt={}\ndn=ezio's laptop\nos=macos\nzz=later",
        known.encode()
    );
    let reply = [BROADCAST_REPLY, body.as_bytes()].concat();
    assert_eq!(
        parse_reply(&reply),
        Some((
            47821,
            presence(PresenceAdvert {
                known: Some(known),
                name: Some("ezio's laptop".into()),
                os: Some(OsKind::MacOs),
            })
        ))
    );
    let quiet = [BROADCAST_REPLY, b"47821\nkt=nothex"].concat();
    assert_eq!(
        parse_reply(&quiet),
        Some((47821, presence(PresenceAdvert::default())))
    );
}
