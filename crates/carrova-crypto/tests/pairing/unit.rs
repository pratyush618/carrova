use std::collections::HashSet;

use super::code::{NAMEPLATE_COUNT, WORD_COUNT, uniform_below};
use super::keys::MacKey;
use super::wordlist::{WORDLIST_LEN, WORDS};
use super::*;

const EXPORTER: [u8; EXPORTER_LEN] = [7; EXPORTER_LEN];
const SENDER_CERT: &[u8] = b"sender certificate DER";
const RECEIVER_CERT: &[u8] = b"receiver certificate DER";

fn code(s: &str) -> PairingCode {
    s.parse().unwrap()
}

fn binding() -> ChannelBinding {
    ChannelBinding::new(EXPORTER, SENDER_CERT, RECEIVER_CERT)
}

/// Runs SPAKE2 between a sender holding `sender_code` and a receiver holding
/// `receiver_code`; returns (sender keys, receiver keys).
fn pair(sender_code: &PairingCode, receiver_code: &PairingCode) -> (SessionKeys, SessionKeys) {
    let (sender, to_receiver) = Pake::start(sender_code, PakeRole::Sender);
    let (receiver, to_sender) = Pake::start(receiver_code, PakeRole::Receiver);
    (
        sender.finish(&to_sender).unwrap(),
        receiver.finish(&to_receiver).unwrap(),
    )
}

/// Both confirmations as the protocol runs them, over the bindings each
/// side sees.
fn confirm_both(
    sender: &SessionKeys,
    sender_view: &ChannelBinding,
    receiver: &SessionKeys,
    receiver_view: &ChannelBinding,
) -> (Result<()>, Result<()>) {
    let from_sender = sender.confirm(PakeRole::Sender, sender_view).unwrap();
    let from_receiver = receiver.confirm(PakeRole::Receiver, receiver_view).unwrap();
    (
        receiver.verify(PakeRole::Sender, receiver_view, &from_sender),
        sender.verify(PakeRole::Receiver, sender_view, &from_receiver),
    )
}

fn secret(byte: u8) -> ResumeSecret {
    ResumeSecret::from_bytes([byte; RESUME_SECRET_LEN])
}

// ------------------------------------------------------------- word list

#[test]
fn wordlist_is_1296_sorted_unique_lowercase_words_with_unique_3_letter_prefixes() {
    assert_eq!(WORDS.len(), 1296);
    assert!(
        WORDS
            .iter()
            .all(|w| w.len() >= MIN_WORD_PREFIX && w.bytes().all(|b| b.is_ascii_lowercase()))
    );
    assert!(WORDS.windows(2).all(|p| p[0] < p[1]), "sorted and unique");
    let prefixes: HashSet<_> = WORDS.iter().map(|w| &w[..MIN_WORD_PREFIX]).collect();
    assert_eq!(prefixes.len(), WORDS.len());
}

// ------------------------------------------------------------- code text

#[test]
fn generated_codes_roundtrip_through_display_and_parse_with_identical_password() {
    for _ in 0..1000 {
        let generated = PairingCode::generate().unwrap();
        let shown = generated.to_string();
        let parsed: PairingCode = shown.parse().unwrap();
        assert_eq!(parsed, generated);
        assert_eq!(parsed.to_string(), shown);
        assert_eq!(*parsed.password_bytes(), *generated.password_bytes());
    }
}

#[test]
fn display_is_nameplate_and_words_joined_by_dashes() {
    assert_eq!(code("7 LEMON zebra").to_string(), "7-lemon-zebra");
}

#[test]
fn spelling_variants_parse_to_the_same_code_and_password() {
    let canonical = code("7-lemon-zebra");
    for variant in [
        "7 Lemon ZEBRA",
        "  7-lemon-zebra ",
        "7 - lemon\tzebra",
        "007-lemon-zebra",
        "7--lemon  zebra",
        "7-lem-zeb",
        "7-LEMO-zebr",
    ] {
        let parsed = code(variant);
        assert_eq!(parsed, canonical, "{variant:?}");
        assert_eq!(*parsed.password_bytes(), *canonical.password_bytes());
    }
}

#[test]
fn password_encodes_nameplate_and_word_indices() {
    let lemon = WORDS.iter().position(|w| *w == "lemon").unwrap();
    let zebra = WORDS.iter().position(|w| *w == "zebra").unwrap();
    let mut expected = PASSWORD_LABEL.to_vec();
    for v in [7, lemon, zebra] {
        expected.extend_from_slice(&u16::try_from(v).unwrap().to_be_bytes());
    }
    assert_eq!(code("7-lemon-zebra").password_bytes().as_slice(), expected);
}

#[test]
fn different_codes_have_different_passwords() {
    let base = code("7-lemon-zebra").password_bytes();
    for other in ["8-lemon-zebra", "7-zebra-lemon", "7-lemon-yoyo"] {
        assert_ne!(*code(other).password_bytes(), *base, "{other}");
    }
}

#[test]
fn malformed_codes_are_rejected_with_typed_errors() {
    let cases = [
        ("", CodeError::Shape),
        ("   ", CodeError::Shape),
        ("7", CodeError::Shape),
        ("7-lemon", CodeError::Shape),
        ("7-lemon-zebra-yoyo", CodeError::Shape),
        ("lemon-zebra", CodeError::Shape),
        ("0-lemon-zebra", CodeError::Nameplate),
        ("1000-lemon-zebra", CodeError::Nameplate),
        ("70000-lemon-zebra", CodeError::Nameplate),
        ("+7-lemon-zebra", CodeError::Nameplate),
        ("x-lemon-zebra", CodeError::Nameplate),
        ("7-lemonade-zebra", CodeError::UnknownWord { position: 1 }),
        ("7-lemon-qqqq", CodeError::UnknownWord { position: 2 }),
        ("7-le-zebra", CodeError::UnknownWord { position: 1 }),
        ("7-lemon-zébra", CodeError::UnknownWord { position: 2 }),
    ];
    for (input, expected) in cases {
        match input.parse::<PairingCode>() {
            Err(PairingError::InvalidCode(e)) => assert_eq!(e, expected, "{input:?}"),
            other => panic!("{input:?}: expected {expected:?}, got {other:?}"),
        }
    }
}

#[test]
fn parse_errors_never_echo_the_typed_words() {
    for input in [
        "7-lemon-qqqq",
        "7-qqqq-zebra",
        "7-lemon-zebra-yoyo",
        "0-lemon-zebra",
    ] {
        let err = input.parse::<PairingCode>().unwrap_err();
        let text = format!("{err} {err:?}");
        for word in ["lemon", "zebra", "qqqq", "yoyo"] {
            assert!(!text.contains(word), "{input:?} leaked {word:?}: {text}");
        }
    }
}

#[test]
fn code_debug_shows_nameplate_but_not_words() {
    let shown = format!("{:?}", code("7-lemon-zebra"));
    assert!(shown.contains('7'), "{shown}");
    assert!(
        !shown.contains("lemon") && !shown.contains("zebra"),
        "{shown}"
    );
}

#[test]
fn generated_nameplates_and_words_cover_their_whole_range() {
    let nameplates: HashSet<_> = (0..30_000)
        .map(|_| uniform_below(NAMEPLATE_COUNT).unwrap())
        .collect();
    assert_eq!(nameplates.len(), usize::from(NAMEPLATE_COUNT.get()));
    assert!(nameplates.iter().all(|n| *n < NAMEPLATE_COUNT.get()));

    let words: HashSet<_> = (0..40_000)
        .map(|_| uniform_below(WORD_COUNT).unwrap())
        .collect();
    assert_eq!(words.len(), WORDLIST_LEN);

    let generated: HashSet<_> = (0..200)
        .map(|_| PairingCode::generate().unwrap().nameplate().get())
        .collect();
    assert!(generated.iter().all(|n| NAMEPLATES.contains(n)));
    assert!(generated.len() > 150, "nameplates should vary");
}

// ------------------------------------------------------------- PAKE + confirm

#[test]
fn matching_codes_derive_equal_keys_and_both_confirmations_verify() {
    let (sender, receiver) = pair(&code("7-lemon-zebra"), &code("7 LEM zeb"));
    let (at_receiver, at_sender) = confirm_both(&sender, &binding(), &receiver, &binding());
    at_receiver.unwrap();
    at_sender.unwrap();
    assert_eq!(
        sender.resume_secret().expose_bytes(),
        receiver.resume_secret().expose_bytes()
    );
}

#[test]
fn wrong_code_fails_confirmation_on_both_sides() {
    let (sender, receiver) = pair(&code("7-lemon-zebra"), &code("7-lemon-yoyo"));
    let (at_receiver, at_sender) = confirm_both(&sender, &binding(), &receiver, &binding());
    assert!(matches!(at_receiver, Err(PairingError::ConfirmationFailed)));
    assert!(matches!(at_sender, Err(PairingError::ConfirmationFailed)));
}

#[test]
fn different_nameplate_with_same_words_fails_confirmation() {
    let (sender, receiver) = pair(&code("7-lemon-zebra"), &code("8-lemon-zebra"));
    let (at_receiver, _) = confirm_both(&sender, &binding(), &receiver, &binding());
    assert!(matches!(at_receiver, Err(PairingError::ConfirmationFailed)));
}

#[test]
fn man_in_the_middle_with_different_tls_session_or_certificate_fails() {
    let c = code("7-lemon-zebra");
    let mitm_views = [
        ChannelBinding::new([8; EXPORTER_LEN], SENDER_CERT, RECEIVER_CERT),
        ChannelBinding::new(EXPORTER, b"attacker cert", RECEIVER_CERT),
        ChannelBinding::new(EXPORTER, SENDER_CERT, b"attacker cert"),
    ];
    for receiver_view in &mitm_views {
        let (sender, receiver) = pair(&c, &c);
        let (at_receiver, at_sender) = confirm_both(&sender, &binding(), &receiver, receiver_view);
        assert!(matches!(at_receiver, Err(PairingError::ConfirmationFailed)));
        assert!(matches!(at_sender, Err(PairingError::ConfirmationFailed)));
    }
}

#[test]
fn reflected_confirmation_is_rejected() {
    let c = code("7-lemon-zebra");
    let (sender, receiver) = pair(&c, &c);
    let b = binding();
    let own_sender = sender.confirm(PakeRole::Sender, &b).unwrap();
    assert!(sender.verify(PakeRole::Receiver, &b, &own_sender).is_err());
    let own_receiver = receiver.confirm(PakeRole::Receiver, &b).unwrap();
    assert!(
        receiver
            .verify(PakeRole::Sender, &b, &own_receiver)
            .is_err()
    );
}

#[test]
fn tampered_confirmation_is_rejected() {
    let c = code("7-lemon-zebra");
    let (sender, receiver) = pair(&c, &c);
    let mut forged = sender.confirm(PakeRole::Sender, &binding()).unwrap();
    forged.0[0] ^= 1;
    assert!(
        receiver
            .verify(PakeRole::Sender, &binding(), &forged)
            .is_err()
    );
}

#[test]
fn pake_message_from_the_same_side_or_garbage_is_rejected() {
    let c = code("7-lemon-zebra");
    let (sender, _) = Pake::start(&c, PakeRole::Sender);
    let (_, other_sender_msg) = Pake::start(&c, PakeRole::Sender);
    assert!(matches!(
        sender.finish(&other_sender_msg),
        Err(PairingError::InvalidPakeMessage)
    ));
    let (receiver, _) = Pake::start(&c, PakeRole::Receiver);
    assert!(matches!(
        receiver.finish(&PakeMessage::from_bytes(vec![0x41; 7])),
        Err(PairingError::InvalidPakeMessage)
    ));
}

#[test]
fn every_pairing_derives_fresh_keys() {
    let c = code("7-lemon-zebra");
    let (first, _) = pair(&c, &c);
    let (second, _) = pair(&c, &c);
    assert_ne!(
        first.resume_secret().expose_bytes(),
        second.resume_secret().expose_bytes()
    );
}

// ------------------------------------------------------------- resume

fn nonce(byte: u8) -> TagNonce {
    TagNonce([byte; TAG_NONCE_LEN])
}

#[test]
fn resume_tag_is_stable_per_secret_and_nonce_and_differs_otherwise() {
    let tag = |s: u8, n: u8| secret(s).tag_for(TagPurpose::Discovery, &nonce(n)).unwrap();
    assert_eq!(tag(1, 1), tag(1, 1));
    assert_ne!(tag(1, 1), tag(2, 1));
    assert_ne!(tag(1, 1), tag(1, 2), "a fresh nonce must rotate the tag");
}

#[test]
fn resume_tag_is_recognised_only_by_its_secret_and_nonce() {
    let n = nonce(5);
    let tag = secret(1).tag_for(TagPurpose::Discovery, &n).unwrap();
    assert!(secret(1).recognises(TagPurpose::Discovery, &n, &tag));
    assert!(!secret(2).recognises(TagPurpose::Discovery, &n, &tag));
    assert!(!secret(1).recognises(TagPurpose::Discovery, &nonce(6), &tag));
}

#[test]
fn hello_tag_differs_from_the_discovery_tag_and_is_not_recognised_as_one() {
    let n = nonce(7);
    let advert = secret(1).tag_for(TagPurpose::Discovery, &n).unwrap();
    let hello = secret(1).tag_for(TagPurpose::Hello, &n).unwrap();
    assert_ne!(advert, hello);
    assert!(secret(1).recognises(TagPurpose::Hello, &n, &hello));
    assert!(!secret(1).recognises(TagPurpose::Hello, &n, &advert));
    assert!(!secret(1).recognises(TagPurpose::Discovery, &n, &hello));
}

#[test]
fn random_tag_nonces_differ_and_roundtrip_as_hex() {
    let a = TagNonce::random().unwrap();
    assert_ne!(a, TagNonce::random().unwrap());
    assert_eq!(a.to_string().len(), 2 * TAG_NONCE_LEN);
    assert_eq!(a.to_string().parse::<TagNonce>().unwrap(), a);
}

#[test]
fn resume_proof_verifies_for_same_secret_role_and_binding() {
    let b = binding();
    let proof = secret(1).prove(PakeRole::Receiver, &b).unwrap();
    secret(1).verify(PakeRole::Receiver, &b, &proof).unwrap();
}

#[test]
fn resume_proof_fails_for_other_binding_role_or_secret() {
    let b = binding();
    let proof = secret(1).prove(PakeRole::Receiver, &b).unwrap();
    let other_binding = ChannelBinding::new([9; EXPORTER_LEN], SENDER_CERT, RECEIVER_CERT);
    assert!(
        secret(1)
            .verify(PakeRole::Receiver, &other_binding, &proof)
            .is_err()
    );
    assert!(secret(1).verify(PakeRole::Sender, &b, &proof).is_err());
    assert!(secret(2).verify(PakeRole::Receiver, &b, &proof).is_err());
}

#[test]
fn pairing_confirmation_is_not_accepted_as_resume_proof() {
    let c = code("7-lemon-zebra");
    let (sender, receiver) = pair(&c, &c);
    let b = binding();
    let confirmation = sender.confirm(PakeRole::Sender, &b).unwrap();
    assert!(
        receiver
            .resume_secret()
            .verify(PakeRole::Sender, &b, &confirmation)
            .is_err()
    );
}

#[test]
fn resume_secret_roundtrips_through_bytes() {
    let c = code("7-lemon-zebra");
    let (sender, _) = pair(&c, &c);
    let original = sender.resume_secret();
    let restored = ResumeSecret::from_bytes(*original.expose_bytes());
    assert_eq!(restored.expose_bytes(), original.expose_bytes());
    assert_eq!(
        restored.tag_for(TagPurpose::Discovery, &nonce(1)).unwrap(),
        original.tag_for(TagPurpose::Discovery, &nonce(1)).unwrap()
    );
}

#[test]
fn resume_tag_and_cert_hash_roundtrip_as_hex() {
    let tag = secret(3).tag_for(TagPurpose::Discovery, &nonce(3)).unwrap();
    assert_eq!(tag.to_string().len(), 2 * RESUME_TAG_LEN);
    assert_eq!(tag.to_string().parse::<ResumeTag>().unwrap(), tag);
    assert_eq!(
        tag.to_string().to_uppercase().parse::<ResumeTag>().unwrap(),
        tag
    );
    let hash = CertHash::of(SENDER_CERT);
    assert_eq!(hash.to_string().parse::<CertHash>().unwrap(), hash);
    assert!("abc".parse::<ResumeTag>().is_err());
    assert!("zz".repeat(RESUME_TAG_LEN).parse::<ResumeTag>().is_err());
}

// ------------------------------------------------------------- redaction

#[test]
fn debug_of_keys_secrets_and_binding_reveals_no_bytes() {
    let c = code("7-lemon-zebra");
    let (sender, _) = pair(&c, &c);
    let resume = sender.resume_secret();
    let hex = format!(
        "{:02x}{:02x}",
        resume.expose_bytes()[0],
        resume.expose_bytes()[1]
    );
    let rendered = [
        format!("{sender:?}"),
        format!("{resume:?}"),
        format!("{:?}", binding()),
        format!("{:?}", Pake::start(&c, PakeRole::Sender).0),
    ];
    for shown in &rendered {
        assert!(!shown.contains(&hex), "{shown}");
        assert!(!shown.contains("lemon") && !shown.contains('['), "{shown}");
    }
    assert_eq!(format!("{resume:?}"), "ResumeSecret(<redacted>)");
}

/// Hex of a byte slice (test helper for known answers).
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Known answers (re-recorded when the `carrova/1/*` labels replaced the
/// working-name labels): the confirmation and resume-proof bytes must never
/// change.
#[test]
fn hmac_outputs_match_known_answers() {
    let keys = SessionKeys {
        confirm: MacKey::new(&[0x11; 32]),
        resume: secret(0x22),
    };
    let b = binding();
    assert_eq!(
        hex(&keys.confirm(PakeRole::Sender, &b).unwrap().0),
        "a3b50f73565c02ea953e1453276df865cb8e9391688953f2113ac779d8b2ff83"
    );
    assert_eq!(
        hex(&keys.confirm(PakeRole::Receiver, &b).unwrap().0),
        "1c3e0049175191c8ef4ebe290dd17443ab0d04d13193720256765d082c46f488"
    );
    assert_eq!(
        hex(&secret(0x22).prove(PakeRole::Sender, &b).unwrap().0),
        "4dfa09e1b7555c7f7cc0cab01f94ad554c244a83397050f1fdfd36345c2f9348"
    );
}
