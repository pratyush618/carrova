use super::*;

#[test]
fn a_code_is_claimed_once_and_stays_claimed_across_lookups() {
    let mut book = CodeBook::default();
    let code = PairingCode::generate().unwrap();
    let claim = book.claim(&code);
    assert!(!claim.is_taken());
    assert!(claim.take());
    assert!(!claim.take());
    assert!(book.claim(&code).is_taken());
    assert!(!book.claim(&PairingCode::generate().unwrap()).is_taken());
}
