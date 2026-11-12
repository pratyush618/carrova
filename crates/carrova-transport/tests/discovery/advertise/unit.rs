use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::{KnownSecret, Nameplate, ResumeSecret};
use carrova_platform::OsKind;

use super::*;
use crate::discovery::presence::{KnownTags, PresenceAdvert};
use crate::discovery::seek::Subject;
use crate::{BROADCAST_BROWSE, BROADCAST_QUERY, TXT_DEVICE_NAME};

fn pairing_offer() -> Advert {
    Advert::Offer(Offer::new(Subject::Pair(Nameplate::new(7).unwrap())).unwrap())
}

#[test]
fn senders_name_themselves_only_to_browsers_and_only_while_offering_a_code() {
    let np_query = [BROADCAST_QUERY, b"np=7"].concat();
    let pairing = pairing_offer();
    assert_eq!(
        reply(&pairing, &np_query, 40123, Some("box")),
        Some([BROADCAST_REPLY, b"40123"].concat()),
        "pairing replies stay byte-identical for older receivers"
    );
    assert_eq!(
        reply(&pairing, BROADCAST_BROWSE, 40123, Some("box")),
        Some([BROADCAST_REPLY, b"40123 box"].concat())
    );
    assert_eq!(reply(&pairing, BROADCAST_BROWSE, 40123, None), None);
    let resume =
        Advert::Offer(Offer::new(Subject::Resume(ResumeSecret::from_bytes([1; 32]))).unwrap());
    assert_eq!(reply(&resume, BROADCAST_BROWSE, 40123, Some("box")), None);
    assert_eq!(BROADCAST_BROWSE, b"CARROVA/1?browse");
}

#[test]
fn advertised_names_fit_one_txt_entry_on_a_char_boundary() {
    assert_eq!(txt_fit("laptop"), "laptop");
    let long = "é".repeat(200);
    let fitted = txt_fit(&long);
    assert!(TXT_DEVICE_NAME.len() + 1 + fitted.len() <= usize::from(u8::MAX));
    assert!(long.starts_with(fitted) && !fitted.is_empty());
}

#[test]
fn presence_answers_browsers_with_its_txt_fields_after_the_port() {
    let known = KnownTags::fresh(
        &[KnownSecret::from_bytes([1; 32])],
        &DeviceKey::from_bytes(&[2; 32]).public(),
    )
    .unwrap()
    .unwrap();
    let kt = known.encode();
    let quiet = Advert::Presence(PresenceAdvert {
        known: Some(known.clone()),
        ..PresenceAdvert::default()
    });
    assert_eq!(
        reply(&quiet, BROADCAST_BROWSE, 47821, Some("box")),
        Some([BROADCAST_REPLY, format!("47821\nkt={kt}").as_bytes()].concat()),
        "the pairing name never leaks into a presence reply"
    );
    let open = Advert::Presence(PresenceAdvert {
        known: Some(known),
        name: Some("desk".into()),
        os: Some(OsKind::Linux),
    });
    assert_eq!(
        reply(&open, BROADCAST_BROWSE, 47821, None),
        Some(
            [
                BROADCAST_REPLY,
                format!("47821\nkt={kt}\ndn=desk\nos=linux").as_bytes()
            ]
            .concat()
        )
    );
    let np_query = [BROADCAST_QUERY, b"np=7"].concat();
    assert_eq!(reply(&open, &np_query, 47821, None), None);
}
