use carrova_crypto::pairing::KnownSecret;
use carrova_platform::OsKind;

use super::*;
use crate::{TXT_DEVICE_NAME, TXT_KNOWN_TAGS, TXT_OS};

fn secret(n: u8) -> KnownSecret {
    KnownSecret::from_bytes([n; 32])
}

fn tags(secrets: &[KnownSecret]) -> KnownTags {
    KnownTags::fresh(secrets).unwrap().unwrap()
}

#[test]
fn known_peer_recognises_its_tag() {
    let advertised = tags(&[secret(1), secret(2)]);
    let seen = KnownTags::parse(&advertised.encode()).unwrap();
    assert!(seen.made_with(&secret(1)));
    assert!(seen.made_with(&secret(2)));
    assert!(
        !seen.made_with(&secret(3)),
        "a stranger's secret matches nothing"
    );
}

#[test]
fn tags_from_two_adverts_are_unlinkable() {
    let first = tags(&[secret(1)]).encode();
    let second = tags(&[secret(1)]).encode();
    let nonce = 2 * TAG_NONCE_LEN;
    assert_ne!(first[..nonce], second[..nonce], "fresh nonce per advert");
    assert_ne!(first[nonce..], second[nonce..], "so the tags differ too");
}

#[test]
fn kt_keeps_the_first_13_tags_and_fits_one_txt_string() {
    let secrets: Vec<_> = (0..20).map(secret).collect();
    let advertised = tags(&secrets);
    assert!(
        secrets[..MAX_KNOWN_TAGS]
            .iter()
            .all(|s| advertised.made_with(s))
    );
    assert!(
        !secrets[MAX_KNOWN_TAGS..]
            .iter()
            .any(|s| advertised.made_with(s))
    );
    let entry = format!("{TXT_KNOWN_TAGS}={}", advertised.encode());
    assert!(entry.len() <= usize::from(u8::MAX), "{} bytes", entry.len());
    assert_eq!(KnownTags::parse(&advertised.encode()), Some(advertised));
}

#[test]
fn no_known_secret_means_no_kt() {
    assert!(KnownTags::fresh(&[]).unwrap().is_none());
}

#[test]
fn malformed_kt_is_ignored() {
    let one = tags(&[secret(1)]).encode();
    let fourteen = tags(&(0..14).map(secret).collect::<Vec<_>>()).encode() + &one[32..];
    for bad in [
        "",
        &one[..2 * TAG_NONCE_LEN],
        &one[..one.len() - 1],
        &fourteen,
        &format!("é{}", &one[2..]),
    ] {
        assert_eq!(KnownTags::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn debug_hides_nonce_and_tags() {
    let advertised = tags(&[secret(1)]);
    let shown = format!("{advertised:?}");
    assert!(!shown.contains(&advertised.encode()[..8]), "{shown}");
}

#[test]
fn an_advert_reads_back_from_its_txt() {
    let advert = PresenceAdvert {
        known: Some(tags(&[secret(1)])),
        name: Some("desk".into()),
        os: Some(OsKind::Linux),
    };
    let txt = advert.txt();
    let keys: Vec<_> = txt.iter().map(|(k, _)| *k).collect();
    assert_eq!(keys, [TXT_KNOWN_TAGS, TXT_DEVICE_NAME, TXT_OS]);
    let get = |key: &str| txt.iter().find(|(k, _)| *k == key).map(|(_, v)| v.as_str());
    assert_eq!(PresenceAdvert::from_txt(get), advert);
}

#[test]
fn an_advert_without_name_or_os_carries_only_kt() {
    let advert = PresenceAdvert {
        known: Some(tags(&[secret(1)])),
        ..PresenceAdvert::default()
    };
    let keys: Vec<_> = advert.txt().into_iter().map(|(k, _)| k).collect();
    assert_eq!(keys, [TXT_KNOWN_TAGS]);
}
