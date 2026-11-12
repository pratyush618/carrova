use data_encoding::HEXLOWER;

use super::*;
use crate::pairing::{
    KNOWN_SECRET_LEN, KNOWN_TAG_LEN, PairingCode, Pake, PakeRole, ResumeSecret, TAG_NONCE_LEN,
    TagNonce, TagPurpose,
};

fn secret(byte: u8) -> KnownSecret {
    KnownSecret::from_bytes([byte; KNOWN_SECRET_LEN])
}

fn nonce(byte: u8) -> TagNonce {
    TagNonce([byte; TAG_NONCE_LEN])
}

fn tag(s: &KnownSecret, n: &TagNonce) -> KnownTag {
    s.tag_for(KnownPurpose::Discovery, n).unwrap()
}

#[test]
fn known_secret_differs_from_resume_secret() {
    let code: PairingCode = "7-lemon-zebra".parse().unwrap();
    let (sender, to_receiver) = Pake::start(&code, PakeRole::Sender);
    let (receiver, to_sender) = Pake::start(&code, PakeRole::Receiver);
    let sender = sender.finish(&to_sender).unwrap();
    let receiver = receiver.finish(&to_receiver).unwrap();
    assert_eq!(
        sender.known_secret().expose_bytes(),
        receiver.known_secret().expose_bytes()
    );
    assert_ne!(
        sender.known_secret().expose_bytes(),
        sender.resume_secret().expose_bytes()
    );
}

#[test]
fn discovery_tag_is_recognised_only_for_its_secret_and_nonce() {
    let s = secret(1);
    let t = tag(&s, &nonce(9));
    assert_eq!(t, tag(&s, &nonce(9)));
    assert!(s.recognises(KnownPurpose::Discovery, &nonce(9), &t));
    assert!(!s.recognises(KnownPurpose::Discovery, &nonce(8), &t));
    assert!(!secret(2).recognises(KnownPurpose::Discovery, &nonce(9), &t));
}

#[test]
fn discovery_tag_is_not_the_resume_tag_of_the_same_key() {
    let resume = ResumeSecret::from_bytes([1; KNOWN_SECRET_LEN])
        .tag_for(TagPurpose::Discovery, &nonce(9))
        .unwrap();
    assert_ne!(tag(&secret(1), &nonce(9)).0[..], resume.0[..KNOWN_TAG_LEN]);
}

#[test]
fn tag_is_eight_bytes_and_round_trips_as_hex() {
    let t = tag(&secret(1), &nonce(9));
    assert_eq!(t.0.len(), 8);
    assert_eq!(t.to_string().parse::<KnownTag>().unwrap(), t);
}

#[test]
fn debug_output_has_no_secret_or_tag_bytes() {
    let s = secret(0xab);
    let t = tag(&s, &nonce(9));
    let shown = format!("{s:?} {t:?}");
    assert!(!shown.contains("abab"), "{shown}");
    assert!(!shown.contains(&HEXLOWER.encode(&t.0)), "{shown}");
}

/// Known answer: the discovery tag format must never change silently
/// (both peers recompute it).
#[test]
fn discovery_tag_matches_known_answer() {
    assert_eq!(
        tag(&secret(0x33), &nonce(0x44)).to_string(),
        "e6dbf109bef5515a"
    );
}
