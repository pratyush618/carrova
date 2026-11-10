use super::*;

#[test]
fn io_error_round_trips_typed_error() {
    let err = io::Error::other(CryptoError::Truncated);
    assert!(matches!(
        CryptoError::in_io(&err),
        Some(CryptoError::Truncated)
    ));
    assert!(matches!(CryptoError::from(err), CryptoError::Truncated));
    let plain = io::Error::new(io::ErrorKind::NotFound, "x");
    assert!(CryptoError::in_io(&plain).is_none());
    assert!(matches!(CryptoError::from_io(plain), CryptoError::Io(_)));
}
