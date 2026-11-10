use super::*;

fn query(kv: &str) -> Vec<u8> {
    [BROADCAST_QUERY, kv.as_bytes()].concat()
}

fn fixed_proof() -> TagProof {
    TagProof {
        nonce: TagNonce([0xab; TAG_NONCE_LEN]),
        tag: ResumeTag([0x0c; RESUME_TAG_LEN]),
    }
}

/// Wire snapshot: the exact discovery strings senders and receivers
/// exchange (mDNS TXT and broadcast query) must never change.
#[test]
fn discovery_keys_are_byte_identical_on_the_wire() {
    let np = Nameplate::new(42).unwrap();
    let pair = Offer::new(Subject::Pair(np)).unwrap();
    assert_eq!(pair.advert().key().as_str(), "np");
    assert_eq!(pair.advert().value(), "42");
    assert_eq!(Seek::Pairing(np).to_query(), b"CARROVA/1?np=42");
    let value = format!(
        "{}{}",
        "ab".repeat(TAG_NONCE_LEN),
        "0c".repeat(RESUME_TAG_LEN)
    );
    let resume = Seek::Resume(fixed_proof());
    assert_eq!(resume.key().as_str(), "rt");
    assert_eq!(resume.value(), value);
    let offer = Offer::new(Subject::Resume(ResumeSecret::from_bytes([9; 32]))).unwrap();
    assert!(
        !offer.answers(&query(&format!("rt={value}"))),
        "foreign tag"
    );
    let Seek::Resume(advert) = *offer.advert() else {
        panic!("resume offer advertises a resume tag")
    };
    let upper = Seek::Resume(advert).value().to_uppercase();
    assert!(
        offer.answers(&query(&format!("rt={upper}"))),
        "hex is case-insensitive"
    );
    assert!(!pair.answers(&query("np=042")), "non-canonical");
    assert!(!pair.answers(&query(" np=42")));
    assert!(!pair.answers(&query("np= 42")));
    assert!(!pair.answers(b"CARROVA/1!np=42"));
}

#[test]
fn query_names_nameplate_but_not_words() {
    let np = Nameplate::new(7).unwrap();
    let seek = Subject::Pair(np).fresh_seek().unwrap();
    assert_eq!(seek.to_query(), b"CARROVA/1?np=7");
    let offer = Offer::new(Subject::Pair(np)).unwrap();
    assert!(offer.answers(b"CARROVA/1?np=7"));
    assert!(!offer.answers(b"CARROVA/1?np=70"));
}

#[test]
fn resume_queries_rotate_and_are_answered_only_with_the_secret() {
    let secret = ResumeSecret::from_bytes([1; 32]);
    let wanted = Subject::Resume(secret.clone());
    let a = wanted.fresh_seek().unwrap().to_query();
    let b = wanted.fresh_seek().unwrap().to_query();
    assert_ne!(a, b, "every query carries a fresh tag");
    let offer = Offer::new(Subject::Resume(secret)).unwrap();
    assert!(offer.answers(&a) && offer.answers(&b));
    let stranger = Subject::Resume(ResumeSecret::from_bytes([2; 32]));
    assert!(!offer.answers(&stranger.fresh_seek().unwrap().to_query()));
    assert!(!offer.answers(&query("rt=00")));
    assert!(!offer.answers(&query("np=7")));
}

#[test]
fn seek_round_trips_through_its_wire_form() {
    for seek in [
        Seek::Pairing(Nameplate::new(999).unwrap()),
        Seek::Resume(fixed_proof()),
    ] {
        assert_eq!(Seek::parse(seek.key(), &seek.value()), Some(seek));
        assert_eq!(Seek::from_query(&seek.to_query()), Some(seek));
    }
    assert_eq!(Seek::from_query(&query("xx=7")), None);
    assert_eq!(Seek::parse(SeekKey::Nameplate, "0"), None);
}

#[test]
fn tag_proof_round_trips_and_rejects_malformed_values() {
    let p = TagProof::fresh(&ResumeSecret::from_bytes([3; 32]), TagPurpose::Discovery).unwrap();
    assert_eq!(TagProof::parse(&p.encode()), Some(p));
    assert_eq!(TagProof::parse(""), None);
    assert_eq!(TagProof::parse(&"é".repeat(32)), None);
    assert_eq!(TagProof::parse(&"zz".repeat(32)), None);
}
