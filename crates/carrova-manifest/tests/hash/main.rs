//! BLAKE3 hashes: hex text round trip, reference vector and CBOR byte-string encoding.

use carrova_manifest::Blake3Hash;

#[test]
fn hex_round_trip() {
    let h = Blake3Hash::of(b"carrova");
    let s = h.to_string();
    assert_eq!(s.len(), 64);
    assert_eq!(s.parse::<Blake3Hash>().unwrap(), h);
    assert_eq!(s.to_uppercase().parse::<Blake3Hash>().unwrap(), h);
}

#[test]
fn empty_hash_matches_blake3_reference() {
    assert_eq!(
        Blake3Hash::of(b"").to_string(),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );
}

#[test]
fn bad_hex_rejected() {
    assert!("abc".parse::<Blake3Hash>().is_err());
    assert!("zz".repeat(32).parse::<Blake3Hash>().is_err());
    assert!("aa".repeat(33).parse::<Blake3Hash>().is_err());
}

#[test]
fn cbor_encodes_as_byte_string() {
    let h = Blake3Hash::of(b"x");
    let mut buf = Vec::new();
    ciborium::into_writer(&h, &mut buf).unwrap();
    // major type 2 (bytes), length 32 → 0x58 0x20
    assert_eq!(&buf[..2], &[0x58, 0x20]);
    assert_eq!(buf.len(), 34);
    let back: Blake3Hash = ciborium::from_reader(buf.as_slice()).unwrap();
    assert_eq!(back, h);
}
