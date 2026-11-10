//! Signer chains, signature domain separation and key encodings.

use carrova_crypto::sign::{
    KeyCert, KeyRole, Result, SECRET_KEY_LEN, SignContext, SignError, Signature, SignedKeyCert,
    SigningKey, TrustRoots, VerifyingKey, verify_chain,
};

const NOW: u64 = 1_800_000_000;
const DAY: u64 = 86_400;
const INDEX: &[u8] = br#"{"format":1,"version":7}"#;

struct Chain {
    roots: TrustRoots,
    signer: SigningKey,
    cert: SignedKeyCert,
}

fn chain() -> Chain {
    let root = SigningKey::generate().unwrap();
    let signer = SigningKey::generate().unwrap();
    let cert = KeyCert {
        key: signer.verifying_key(),
        role: KeyRole::BundleSigner,
        not_before: NOW - DAY,
        not_after: NOW + DAY,
    }
    .sign(&root)
    .unwrap();
    Chain {
        roots: TrustRoots::new([root.verifying_key()]).unwrap(),
        signer,
        cert,
    }
}

fn verify(
    c: &Chain,
    cert: &SignedKeyCert,
    payload: &[u8],
    sig: &Signature,
    now: u64,
) -> Result<()> {
    verify_chain(&c.roots, cert, SignContext::BundleIndex, payload, sig, now)
}

#[test]
fn valid_chain_verifies() {
    let c = chain();
    let sig = c.signer.sign(SignContext::BundleIndex, INDEX);
    verify(&c, &c.cert, INDEX, &sig, NOW).unwrap();
}

#[test]
fn validity_window_is_inclusive() {
    let c = chain();
    let sig = c.signer.sign(SignContext::BundleIndex, INDEX);
    verify(&c, &c.cert, INDEX, &sig, c.cert.cert.not_before).unwrap();
    verify(&c, &c.cert, INDEX, &sig, c.cert.cert.not_after).unwrap();
}

#[test]
fn tampered_message_is_rejected() {
    let c = chain();
    let sig = c.signer.sign(SignContext::BundleIndex, INDEX);
    let err = verify(&c, &c.cert, br#"{"format":1,"version":8}"#, &sig, NOW).unwrap_err();
    assert_eq!(err, SignError::BadSignature);
}

#[test]
fn tampered_cert_is_rejected() {
    let c = chain();
    let sig = c.signer.sign(SignContext::BundleIndex, INDEX);
    let mut extended = c.cert;
    extended.cert.not_after += 365 * DAY;
    assert_eq!(
        verify(&c, &extended, INDEX, &sig, NOW).unwrap_err(),
        SignError::BadCertSignature
    );

    let attacker = SigningKey::generate().unwrap();
    let mut swapped = c.cert;
    swapped.cert.key = attacker.verifying_key();
    let forged = attacker.sign(SignContext::BundleIndex, INDEX);
    assert_eq!(
        verify(&c, &swapped, INDEX, &forged, NOW).unwrap_err(),
        SignError::BadCertSignature
    );
}

#[test]
fn cert_from_unknown_root_is_rejected() {
    let c = chain();
    let other = chain();
    let sig = other.signer.sign(SignContext::BundleIndex, INDEX);
    let err = verify(&c, &other.cert, INDEX, &sig, NOW).unwrap_err();
    assert_eq!(err, SignError::UnknownRoot(other.cert.root));
}

#[test]
fn any_listed_root_is_trusted() {
    let c = chain();
    let other = chain();
    let roots = TrustRoots::new(c.roots.iter().chain(other.roots.iter()).copied()).unwrap();
    let sig = other.signer.sign(SignContext::BundleIndex, INDEX);
    verify_chain(
        &roots,
        &other.cert,
        SignContext::BundleIndex,
        INDEX,
        &sig,
        NOW,
    )
    .unwrap();
}

#[test]
fn expired_cert_is_rejected() {
    let c = chain();
    let sig = c.signer.sign(SignContext::BundleIndex, INDEX);
    let now = c.cert.cert.not_after + 1;
    let err = verify(&c, &c.cert, INDEX, &sig, now).unwrap_err();
    assert_eq!(
        err,
        SignError::Expired {
            not_after: c.cert.cert.not_after,
            now
        }
    );
}

#[test]
fn not_yet_valid_cert_is_rejected() {
    let c = chain();
    let sig = c.signer.sign(SignContext::BundleIndex, INDEX);
    let now = c.cert.cert.not_before - 1;
    let err = verify(&c, &c.cert, INDEX, &sig, now).unwrap_err();
    assert!(matches!(err, SignError::NotYetValid { .. }), "{err}");
}

#[test]
fn inverted_validity_cannot_be_signed() {
    let root = SigningKey::generate().unwrap();
    let cert = KeyCert {
        key: root.verifying_key(),
        role: KeyRole::BundleSigner,
        not_before: NOW,
        not_after: NOW - 1,
    };
    assert_eq!(cert.sign(&root).unwrap_err(), SignError::InvertedValidity);
}

#[test]
fn cert_signature_does_not_verify_as_index_signature() {
    let c = chain();
    // Root signature over the cert bytes, replayed as an index signature
    // by the root itself: the context tag differs.
    let root_key = c.cert.root;
    let payload = c.cert.cert.canonical_bytes();
    root_key
        .verify(SignContext::KeyCert, &payload, &c.cert.sig)
        .unwrap();
    assert_eq!(
        root_key
            .verify(SignContext::BundleIndex, &payload, &c.cert.sig)
            .unwrap_err(),
        SignError::BadSignature
    );
}

#[test]
fn index_signature_does_not_verify_as_cert_signature() {
    let root = SigningKey::generate().unwrap();
    let cert = KeyCert {
        key: SigningKey::generate().unwrap().verifying_key(),
        role: KeyRole::BundleSigner,
        not_before: NOW - DAY,
        not_after: NOW + DAY,
    };
    // A root tricked into signing the cert bytes as a bundle index.
    let wrong_context = SignedKeyCert {
        cert,
        root: root.verifying_key(),
        sig: root.sign(SignContext::BundleIndex, &cert.canonical_bytes()),
    };
    let roots = TrustRoots::new([root.verifying_key()]).unwrap();
    let sig = Signature::from_bytes(&[0; 64]);
    let err = verify_chain(
        &roots,
        &wrong_context,
        SignContext::BundleIndex,
        INDEX,
        &sig,
        NOW,
    )
    .unwrap_err();
    assert_eq!(err, SignError::BadCertSignature);
}

#[test]
fn bundle_signer_cannot_sign_certificates() {
    let c = chain();
    let sig = c.signer.sign(SignContext::KeyCert, INDEX);
    let err = verify_chain(&c.roots, &c.cert, SignContext::KeyCert, INDEX, &sig, NOW).unwrap_err();
    assert_eq!(
        err,
        SignError::WrongRole {
            role: KeyRole::BundleSigner,
            context: SignContext::KeyCert
        }
    );
}

#[test]
fn context_tags_are_distinct_and_nul_terminated() {
    let tags = [SignContext::KeyCert.tag(), SignContext::BundleIndex.tag()];
    for tag in tags {
        assert_eq!(tag.last(), Some(&0));
        assert_eq!(tag.iter().filter(|b| **b == 0).count(), 1);
    }
    assert_ne!(tags[0], tags[1]);
}

#[test]
fn signing_key_debug_redacts_secret() {
    let key = SigningKey::generate().unwrap();
    let secret = key.to_secret_hex();
    let shown = format!("{key:?}");
    assert_eq!(shown, "SigningKey(***)");
    assert!(!shown.contains(secret.as_str()));
}

#[test]
fn secret_hex_round_trips() {
    let key = SigningKey::generate().unwrap();
    let back = SigningKey::from_secret_hex(&key.to_secret_hex()).unwrap();
    assert_eq!(back.verifying_key(), key.verifying_key());
    assert!(SigningKey::from_secret_hex("abcd").is_err());
}

#[test]
fn public_values_round_trip_through_hex_and_serde() {
    let c = chain();
    let key = c.signer.verifying_key();
    let text = key.to_string();
    assert_eq!(text.len(), 64);
    assert_eq!(text, text.to_lowercase());
    assert_eq!(text.parse::<VerifyingKey>().unwrap(), key);
    assert_eq!(text.to_uppercase().parse::<VerifyingKey>().unwrap(), key);
    assert_eq!(
        "zz".repeat(32).parse::<VerifyingKey>().unwrap_err(),
        SignError::InvalidHex("public key")
    );

    let sig = c.signer.sign(SignContext::BundleIndex, INDEX);
    assert_eq!(sig.to_string().parse::<Signature>().unwrap(), sig);

    let json = serde_json::to_string(&c.cert).unwrap();
    assert!(json.contains(&format!("\"{key}\"")), "{json}");
    assert!(json.contains("\"bundle_signer\""), "{json}");
    let back: SignedKeyCert = serde_json::from_str(&json).unwrap();
    assert_eq!(back, c.cert);
}

#[test]
fn trust_roots_are_non_empty_and_deduplicated() {
    assert_eq!(TrustRoots::new([]).unwrap_err(), SignError::NoTrustRoots);
    assert!(serde_json::from_str::<TrustRoots>("[]").is_err());
    let key = SigningKey::generate().unwrap().verifying_key();
    let roots = TrustRoots::new([key, key]).unwrap();
    assert_eq!(roots.iter().count(), 1);
    let json = serde_json::to_string(&roots).unwrap();
    assert_eq!(serde_json::from_str::<TrustRoots>(&json).unwrap(), roots);
}

#[test]
fn canonical_cert_bytes_are_fixed_layout() {
    let key = SigningKey::from_seed(&[7; SECRET_KEY_LEN]).verifying_key();
    let cert = KeyCert {
        key,
        role: KeyRole::BundleSigner,
        not_before: 1,
        not_after: 0x0102_0304_0506_0708,
    };
    let bytes = cert.canonical_bytes();
    assert_eq!(bytes[0], 1);
    assert_eq!(&bytes[1..33], key.as_bytes());
    assert_eq!(bytes[33..41], 1u64.to_be_bytes());
    assert_eq!(bytes[41..], [1, 2, 3, 4, 5, 6, 7, 8]);
}
