use carrova_crypto::identity::DeviceKey;

use super::*;

fn device(name: &str, known: Option<u8>, answered_ms: u32) -> NearbyDevice {
    NearbyDevice {
        name: name.into(),
        os: None,
        known: known.map(|seed| DeviceKey::from_bytes(&[seed; 32]).public()),
        offers_code: false,
        answered_ms,
    }
}

#[test]
fn a_known_device_seen_twice_is_listed_once_at_its_first_answer() {
    let found = [
        device("desk", Some(1), 40),
        device("stranger", None, 80),
        device("desk", Some(1), 900),
        device("other stranger", None, 950),
    ];
    let listed: Vec<_> = once_per_known_device(found)
        .into_iter()
        .map(|d| (d.name, d.answered_ms))
        .collect();
    assert_eq!(
        listed,
        [
            ("desk".to_owned(), 40),
            ("stranger".to_owned(), 80),
            ("other stranger".to_owned(), 950)
        ]
    );
}
