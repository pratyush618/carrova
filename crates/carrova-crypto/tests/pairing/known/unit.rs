use data_encoding::HEXLOWER;

use super::*;
use crate::identity::{DeviceKey, DevicePublic};
use crate::pairing::{
    ChannelBinding, EXPORTER_LEN, KNOWN_SECRET_LEN, KNOWN_TAG_LEN, LINK_TAG_LEN, PairingCode, Pake,
    PakeRole, ResumeSecret, TAG_NONCE_LEN, TagNonce, TagPurpose,
};

fn secret(byte: u8) -> KnownSecret {
    KnownSecret::from_bytes([byte; KNOWN_SECRET_LEN])
}

fn nonce(byte: u8) -> TagNonce {
    TagNonce([byte; TAG_NONCE_LEN])
}

fn device(seed: u8) -> DevicePublic {
    DeviceKey::from_bytes(&[seed; 32]).public()
}

fn tag(s: &KnownSecret, n: &TagNonce) -> KnownTag {
    s.tag_for(KnownPurpose::Discovery, n, &device(1)).unwrap()
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
    let a = device(1);
    assert!(s.recognises(KnownPurpose::Discovery, &nonce(9), &a, &t));
    assert!(!s.recognises(KnownPurpose::Discovery, &nonce(8), &a, &t));
    assert!(!secret(2).recognises(KnownPurpose::Discovery, &nonce(9), &a, &t));
}

#[test]
fn discovery_tag_names_its_advertiser() {
    let s = secret(1);
    let t = tag(&s, &nonce(9));
    assert!(
        !s.recognises(KnownPurpose::Discovery, &nonce(9), &device(2), &t),
        "the peer sharing the secret did not make it: no self-recognition"
    );
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
/// (both peers recompute it). `HMAC-SHA256(secret, label ‖ nonce ‖
/// advertiser public key)[..8]`.
#[test]
fn discovery_tag_matches_known_answer() {
    let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(&[0x33; 32]).unwrap();
    hmac::Mac::update(&mut mac, b"carrova/1/known-discovery");
    hmac::Mac::update(&mut mac, &[0x44; TAG_NONCE_LEN]);
    hmac::Mac::update(&mut mac, device(1).as_bytes());
    let full = hmac::Mac::finalize(mac).into_bytes();
    assert_eq!(
        tag(&secret(0x33), &nonce(0x44)).0[..],
        full[..KNOWN_TAG_LEN]
    );
    assert_eq!(
        tag(&secret(0x33), &nonce(0x44)).to_string(),
        "6b07b4a2f5bfad76"
    );
}

// ------------------------------------------------------------------ link

fn binding(exporter: u8) -> ChannelBinding {
    ChannelBinding::new([exporter; EXPORTER_LEN], b"sender cert", b"receiver cert")
}

#[test]
fn link_tag_roundtrip() {
    let s = secret(1);
    let t = s.link_tag(&nonce(9), &device(1)).unwrap();
    assert!(s.recognises_link(&nonce(9), &device(1), &t));
    assert!(!s.recognises_link(&nonce(8), &device(1), &t));
    assert!(!secret(2).recognises_link(&nonce(9), &device(1), &t));
}

#[test]
fn link_tag_bound_to_dialer() {
    let s = secret(1);
    let t = s.link_tag(&nonce(9), &device(1)).unwrap();
    assert!(!s.recognises_link(&nonce(9), &device(2), &t));
}

#[test]
fn link_tag_not_discovery_tag() {
    let s = secret(1);
    let full = s
        .tag_mac(KnownPurpose::Discovery, &nonce(9), &device(1))
        .unwrap()
        .finalize()
        .into_bytes();
    let mut forged = [0u8; LINK_TAG_LEN];
    forged.copy_from_slice(&full[..LINK_TAG_LEN]);
    assert!(!s.recognises_link(&nonce(9), &device(1), &LinkTag(forged)));
}

/// Known answer: `HMAC-SHA256(secret, "carrova/1/known-link" ‖ nonce ‖
/// dialer public key)[..16]` (both peers recompute it).
#[test]
fn link_tag_matches_construction() {
    let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(&[0x33; 32]).unwrap();
    hmac::Mac::update(&mut mac, b"carrova/1/known-link");
    hmac::Mac::update(&mut mac, &[0x44; TAG_NONCE_LEN]);
    hmac::Mac::update(&mut mac, device(1).as_bytes());
    let full = hmac::Mac::finalize(mac).into_bytes();
    let t = secret(0x33).link_tag(&nonce(0x44), &device(1)).unwrap();
    assert_eq!(t.0[..], full[..LINK_TAG_LEN]);
}

#[test]
fn link_tag_debug_is_redacted() {
    let t = secret(1).link_tag(&nonce(9), &device(1)).unwrap();
    let shown = format!("{t:?}");
    assert_eq!(shown, "LinkTag(<redacted>)");
}

#[test]
fn link_proof_roundtrip() {
    let s = secret(1);
    let proof = s.prove(PakeRole::Sender, &binding(7)).unwrap();
    assert!(s.verify(PakeRole::Sender, &binding(7), &proof).is_ok());
    assert!(
        secret(2)
            .verify(PakeRole::Sender, &binding(7), &proof)
            .is_err()
    );
}

#[test]
fn link_proof_rejects_reflection() {
    let s = secret(1);
    let mine = s.prove(PakeRole::Receiver, &binding(7)).unwrap();
    assert!(matches!(
        s.verify(PakeRole::Sender, &binding(7), &mine),
        Err(PairingError::ConfirmationFailed)
    ));
}

#[test]
fn link_proof_bound_to_binding() {
    let s = secret(1);
    let proof = s.prove(PakeRole::Sender, &binding(7)).unwrap();
    assert!(s.verify(PakeRole::Sender, &binding(8), &proof).is_err());
}

#[test]
fn link_proof_is_not_a_resume_proof_of_the_same_key() {
    let link = secret(1).prove(PakeRole::Sender, &binding(7)).unwrap();
    let resume = ResumeSecret::from_bytes([1; KNOWN_SECRET_LEN]);
    assert!(resume.verify(PakeRole::Sender, &binding(7), &link).is_err());
}

/// Known answer: `HMAC-SHA256(secret, "carrova/1/link-confirm" ‖ role
/// identity ‖ binding)`.
#[test]
fn link_proof_matches_construction() {
    let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(&[0x33; 32]).unwrap();
    hmac::Mac::update(&mut mac, b"carrova/1/link-confirm");
    hmac::Mac::update(&mut mac, PakeRole::Receiver.identity());
    hmac::Mac::update(&mut mac, binding(7).as_bytes());
    let full = hmac::Mac::finalize(mac).into_bytes();
    let proof = secret(0x33).prove(PakeRole::Receiver, &binding(7)).unwrap();
    assert_eq!(proof.0[..], full[..]);
}
