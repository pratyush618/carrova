use std::net::{Ipv4Addr, SocketAddrV4};

use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::KnownSecret;
use carrova_platform::OsKind;

use super::*;
use crate::discovery::presence::KnownTags;

fn at(last: u8, port: u16) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, last), port)
}

fn everyone(name: &str, known: Option<KnownTags>) -> Announced {
    Announced {
        advert: PresenceAdvert {
            known,
            name: Some(name.into()),
            os: Some(OsKind::Linux),
        },
        offers_code: false,
    }
}

#[test]
fn two_untagged_presences_with_the_same_name_are_two_devices() {
    let desk = everyone("desk", None);
    assert_ne!(
        same_device(at(2, 47821), &desk),
        same_device(at(3, 47821), &desk)
    );
}

#[test]
fn one_advert_answering_on_several_addresses_is_one_device() {
    let me = DeviceKey::from_bytes(&[2; 32]).public();
    let tags = KnownTags::fresh(&[KnownSecret::from_bytes([1; 32])], &me).unwrap();
    let tagged = everyone("desk", tags);
    assert_eq!(
        same_device(at(2, 47821), &tagged),
        same_device(at(3, 47821), &tagged)
    );
    let offer = Announced {
        offers_code: true,
        ..everyone("desk", None)
    };
    assert_eq!(
        same_device(at(2, 40123), &offer),
        same_device(at(3, 40123), &offer)
    );
}
