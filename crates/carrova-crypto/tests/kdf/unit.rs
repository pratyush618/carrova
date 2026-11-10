use super::*;
use crate::NoncePrefix;

/// Cheap parameters for tests (passes the read-side `validate`).
const TEST_KDF: KdfParams = KdfParams {
    m_kib: 8 * 1024,
    t: 1,
    p: 1,
};

#[test]
fn params_validation() {
    assert!(KdfParams::DEFAULT.validate_for_write().is_ok());
    assert!(KdfParams::FLOOR.validate_for_write().is_ok());
    assert!(KdfParams::CEILING.validate().is_ok());
    assert!(KdfParams::CEILING.validate_for_write().is_ok());
    assert!(TEST_KDF.validate().is_ok());
    assert!(TEST_KDF.validate_for_write().is_err());

    let with = |m_kib, t, p| KdfParams { m_kib, t, p };
    let c = KdfParams::CEILING;
    // Reader ceiling.
    for bad in [
        with(c.m_kib + 1, 3, 1),
        with(1024, c.t + 1, 1),
        with(1024, 3, c.p + 1),
        with(u32::MAX, u32::MAX, u32::MAX),
    ] {
        assert!(
            matches!(bad.validate(), Err(CryptoError::InvalidParams(_))),
            "{bad:?}"
        );
    }
    // Zero / below Argon2 minimum.
    for bad in [
        with(0, 3, 1),
        with(1024, 0, 1),
        with(1024, 3, 0),
        with(7, 1, 1),
        with(31, 1, 4),
    ] {
        assert!(
            matches!(bad.validate(), Err(CryptoError::InvalidParams(_))),
            "{bad:?}"
        );
    }
    // Writer floor.
    let f = KdfParams::FLOOR;
    for bad in [with(f.m_kib - 1, f.t, 1), with(f.m_kib, f.t - 1, 1)] {
        assert!(bad.validate().is_ok());
        assert!(
            matches!(bad.validate_for_write(), Err(CryptoError::InvalidParams(_))),
            "{bad:?}"
        );
    }
    // derive_key enforces the reader bounds before allocating anything.
    let pass = Passphrase::new("x".into());
    assert!(matches!(
        derive_key(&pass, &Salt([0; SALT_LEN]), with(c.m_kib * 2, 1, 1)),
        Err(CryptoError::InvalidParams(_))
    ));
}

#[test]
fn kdf_is_deterministic_and_salted() {
    let pass = Passphrase::new("pw".into());
    let a = derive_key(&pass, &Salt([1; SALT_LEN]), TEST_KDF).unwrap();
    let b = derive_key(&pass, &Salt([1; SALT_LEN]), TEST_KDF).unwrap();
    let c = derive_key(&pass, &Salt([2; SALT_LEN]), TEST_KDF).unwrap();
    let d = derive_key(&pass, &Salt([1; SALT_LEN]), KdfParams { t: 2, ..TEST_KDF }).unwrap();
    assert_eq!(*a.0, *b.0);
    assert_ne!(*a.0, *c.0);
    assert_ne!(*a.0, *d.0);
}

#[test]
fn random_salt_and_nonce_differ() {
    assert_ne!(Salt::random().unwrap(), Salt::random().unwrap());
    assert_ne!(
        NoncePrefix::random().unwrap(),
        NoncePrefix::random().unwrap()
    );
}
