use super::*;

#[test]
fn a_nonce_is_accepted_once_and_the_oldest_is_forgotten_beyond_capacity() {
    let nonces = ResumeNonces::new(2);
    let n = |b: u8| TagNonce([b; 16]);
    assert!(nonces.first_use(n(1)));
    assert!(!nonces.first_use(n(1)));
    assert!(nonces.first_use(n(2)));
    assert!(nonces.first_use(n(3)));
    assert!(!nonces.first_use(n(3)));
    assert!(nonces.first_use(n(1)), "the oldest nonce is forgotten");
}
