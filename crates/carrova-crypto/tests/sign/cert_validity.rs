//! Signer certificate validity cap (`MAX_CERT_VALIDITY`), enforced when
//! signing and when verifying a chain.

use carrova_crypto::sign::{
    CERT_CLOCK_SKEW, KeyCert, KeyRole, MAX_CERT_VALIDITY, MAX_CERT_VALIDITY_DAYS, SignContext,
    SignError, SignedKeyCert, SigningKey, TrustRoots, verify_chain,
};

const NOW: u64 = 1_800_000_000;
const DAY: u64 = 86_400;
const INDEX: &[u8] = br#"{"format":1,"version":7}"#;

struct Chain {
    roots: TrustRoots,
    signer: SigningKey,
    cert: SignedKeyCert,
}

fn verify(c: &Chain) -> Result<(), SignError> {
    let sig = c.signer.sign(SignContext::BundleIndex, INDEX);
    verify_chain(
        &c.roots,
        &c.cert,
        SignContext::BundleIndex,
        INDEX,
        &sig,
        NOW,
    )
}

/// A signer cert spanning `window` seconds, signed by the chain's own root
/// directly (bypassing `KeyCert::sign`'s checks, as a careless issuer
/// would).
fn cert_with_window(window: u64) -> Result<Chain, SignError> {
    let root = SigningKey::generate()?;
    let signer = SigningKey::generate()?;
    let cert = KeyCert {
        key: signer.verifying_key(),
        role: KeyRole::BundleSigner,
        not_before: NOW - DAY,
        not_after: NOW - DAY + window,
    };
    let cert = SignedKeyCert {
        cert,
        root: root.verifying_key(),
        sig: root.sign(SignContext::KeyCert, &cert.canonical_bytes()),
    };
    Ok(Chain {
        roots: TrustRoots::new([root.verifying_key()])?,
        signer,
        cert,
    })
}

#[test]
fn cert_valid_for_longer_than_the_cap_is_rejected() {
    let c = cert_with_window(MAX_CERT_VALIDITY + 1).unwrap();
    assert_eq!(
        verify(&c).unwrap_err(),
        SignError::ValidityTooLong {
            secs: MAX_CERT_VALIDITY + 1,
            max: MAX_CERT_VALIDITY
        }
    );
}

#[test]
fn cert_valid_for_exactly_the_cap_verifies() {
    verify(&cert_with_window(MAX_CERT_VALIDITY).unwrap()).unwrap();
    assert_eq!(
        MAX_CERT_VALIDITY,
        MAX_CERT_VALIDITY_DAYS * DAY + CERT_CLOCK_SKEW
    );
}

#[test]
fn over_long_validity_cannot_be_signed() {
    let root = SigningKey::generate().unwrap();
    let cert = KeyCert {
        key: root.verifying_key(),
        role: KeyRole::BundleSigner,
        not_before: NOW,
        not_after: NOW + MAX_CERT_VALIDITY + 1,
    };
    assert!(matches!(
        cert.sign(&root).unwrap_err(),
        SignError::ValidityTooLong { .. }
    ));
}
