use std::io::{Read, Write};

use super::*;
use crate::{KdfParams, Passphrase, SALT_LEN, Salt, derive_key};

/// Cheap parameters for tests (passes the read-side `validate`).
const TEST_KDF: KdfParams = KdfParams {
    m_kib: 8 * 1024,
    t: 1,
    p: 1,
};
const AAD: &[u8] = b"CARROVA\0 test header";
const NONCE: NoncePrefix = NoncePrefix([7; NONCE_PREFIX_LEN]);

fn key(pass: &str) -> ArchiveKey {
    derive_key(
        &Passphrase::new(pass.into()),
        &Salt([1; SALT_LEN]),
        TEST_KDF,
    )
    .unwrap()
}

fn data(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 % 251) as u8).collect()
}

fn encrypt_with(k: &ArchiveKey, plain: &[u8], aad: &[u8]) -> Vec<u8> {
    let mut w = EncryptWriter::new(Vec::new(), k, &NONCE, aad).unwrap();
    // Odd write sizes exercise buffering across segment boundaries.
    for piece in plain.chunks(10_007) {
        w.write_all(piece).unwrap();
    }
    w.finish().unwrap()
}

fn encrypt(plain: &[u8]) -> Vec<u8> {
    encrypt_with(&key("correct horse"), plain, AAD)
}

fn decrypt_with(k: &ArchiveKey, ct: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let mut r = DecryptReader::new(ct, k, &NONCE, aad)?;
    let mut out = Vec::new();
    r.read_to_end(&mut out)?;
    Ok(out)
}

fn decrypt(ct: &[u8]) -> Result<Vec<u8>> {
    decrypt_with(&key("correct horse"), ct, AAD)
}

fn segments(ct: &[u8]) -> Vec<&[u8]> {
    ct.chunks(FULL_SEGMENT).collect()
}

const SIZES: [usize; 6] = [
    0,
    1,
    SEGMENT_LEN - 1,
    SEGMENT_LEN,
    SEGMENT_LEN + 1,
    3 * SEGMENT_LEN + 7,
];

#[test]
fn round_trip_sizes() {
    let k = key("correct horse");
    for len in SIZES {
        let plain = data(len);
        let ct = encrypt_with(&k, &plain, AAD);
        assert_eq!(ct.len() as u64, ciphertext_len(len as u64), "len {len}");
        assert_eq!(decrypt_with(&k, &ct, AAD).unwrap(), plain, "len {len}");
    }
}

#[test]
fn round_trip_byte_by_byte_reads() {
    let k = key("correct horse");
    let plain = data(SEGMENT_LEN + 3);
    let ct = encrypt_with(&k, &plain, AAD);
    let mut r = DecryptReader::new(&ct[..], &k, &NONCE, AAD).unwrap();
    let mut out = Vec::new();
    let mut b = [0u8; 1];
    while r.read(&mut b).unwrap() == 1 {
        out.push(b[0]);
    }
    assert_eq!(out, plain);
    assert_eq!(r.read(&mut b).unwrap(), 0);
}

#[test]
fn ciphertext_len_layout() {
    assert_eq!(ciphertext_len(0), TAG_LEN as u64);
    assert_eq!(ciphertext_len(1), 1 + TAG_LEN as u64);
    assert_eq!(ciphertext_len(SEGMENT_LEN as u64), FULL_SEGMENT as u64);
    assert_eq!(
        ciphertext_len(SEGMENT_LEN as u64 + 1),
        (FULL_SEGMENT + 1 + TAG_LEN) as u64
    );
}

fn assert_err(r: Result<Vec<u8>>, want: &str) {
    match (r, want) {
        (Err(CryptoError::Decrypt), "decrypt") | (Err(CryptoError::Truncated), "truncated") => {}
        (other, _) => panic!("expected {want}, got {other:?}"),
    }
}

#[test]
fn wrong_passphrase_fails() {
    for len in [0, SEGMENT_LEN, 3 * SEGMENT_LEN + 7] {
        let ct = encrypt(&data(len));
        assert_err(decrypt_with(&key("wrong"), &ct, AAD), "decrypt");
    }
}

#[test]
fn different_aad_fails() {
    let ct = encrypt(&data(100));
    assert_err(
        decrypt_with(&key("correct horse"), &ct, b"CARROVA\0 test headeR"),
        "decrypt",
    );
}

#[test]
fn wrong_nonce_fails() {
    let k = key("correct horse");
    let ct = encrypt_with(&k, &data(100), AAD);
    let mut r = DecryptReader::new(&ct[..], &k, &NoncePrefix([8; NONCE_PREFIX_LEN]), AAD).unwrap();
    let err = r.read_to_end(&mut Vec::new()).unwrap_err();
    assert!(matches!(
        CryptoError::in_io(&err),
        Some(CryptoError::Decrypt)
    ));
}

#[test]
fn flipped_bit_in_any_segment_fails() {
    let ct = encrypt(&data(3 * SEGMENT_LEN + 7));
    assert_eq!(segments(&ct).len(), 4);
    // First byte, middle, and tag of each segment.
    for seg in 0..4 {
        let start = seg * FULL_SEGMENT;
        let end = (start + FULL_SEGMENT).min(ct.len());
        for at in [start, (start + end) / 2, end - 1] {
            let mut bad = ct.clone();
            bad[at] ^= 0x01;
            assert_err(decrypt(&bad), "decrypt");
        }
    }
}

#[test]
fn never_returns_unauthenticated_plaintext() {
    let plain = data(3 * SEGMENT_LEN + 7);
    let mut ct = encrypt(&plain);
    ct[FULL_SEGMENT + 5] ^= 0x80; // corrupt segment 1
    let k = key("correct horse");
    let mut r = DecryptReader::new(&ct[..], &k, &NONCE, AAD).unwrap();
    let mut out = Vec::new();
    let mut buf = vec![0u8; 4096];
    let err = loop {
        match r.read(&mut buf) {
            Ok(0) => panic!("unexpected EOF"),
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) => break e,
        }
    };
    // Only the verified first segment was released.
    assert_eq!(out, plain[..SEGMENT_LEN]);
    assert!(matches!(
        CryptoError::in_io(&err),
        Some(CryptoError::Decrypt)
    ));
    // Poisoned afterwards.
    assert!(r.read(&mut buf).is_err());
}

#[test]
fn truncated_at_segment_boundary() {
    let ct = encrypt(&data(3 * SEGMENT_LEN + 7));
    for keep in [0, FULL_SEGMENT, 2 * FULL_SEGMENT, 3 * FULL_SEGMENT] {
        assert_err(decrypt(&ct[..keep]), "truncated");
    }
    // Exact multiple: dropping the final full segment is also a clean cut.
    let ct = encrypt(&data(2 * SEGMENT_LEN));
    assert_eq!(ct.len(), 2 * FULL_SEGMENT);
    assert_err(decrypt(&ct[..FULL_SEGMENT]), "truncated");
}

#[test]
fn truncated_mid_segment() {
    let ct = encrypt(&data(3 * SEGMENT_LEN + 7));
    for keep in [
        5,
        TAG_LEN + 3,
        FULL_SEGMENT + 100,
        ct.len() - 1,
        3 * FULL_SEGMENT + 3,
    ] {
        let r = decrypt(&ct[..keep]);
        assert!(
            matches!(r, Err(CryptoError::Decrypt | CryptoError::Truncated)),
            "keep {keep}: {r:?}"
        );
    }
    // Empty stream cut inside its only (tag-only) segment.
    let ct = encrypt(&[]);
    assert_err(decrypt(&ct[..TAG_LEN - 1]), "truncated");
}

#[test]
fn missing_finish_is_truncated() {
    let k = key("correct horse");
    let mut w = EncryptWriter::new(Vec::new(), &k, &NONCE, AAD).unwrap();
    w.write_all(&data(2 * SEGMENT_LEN + 1)).unwrap();
    // Steal what was written so far without finishing.
    let partial = w.inner.clone();
    drop(w);
    assert_eq!(partial.len(), 2 * FULL_SEGMENT);
    assert_err(decrypt(&partial), "truncated");
}

#[test]
fn swapped_segments_fail() {
    let ct = encrypt(&data(3 * SEGMENT_LEN + 7));
    let s = segments(&ct);
    let swapped: Vec<u8> = [s[1], s[0], s[2], s[3]].concat();
    assert_err(decrypt(&swapped), "decrypt");
    // Duplicated segment.
    let dup: Vec<u8> = [s[0], s[0], s[1], s[2], s[3]].concat();
    assert_err(decrypt(&dup), "decrypt");
    // Dropped middle segment.
    let dropped: Vec<u8> = [s[0], s[2], s[3]].concat();
    assert_err(decrypt(&dropped), "decrypt");
}

#[test]
fn trailing_garbage_fails() {
    for len in [0, 7, SEGMENT_LEN, 3 * SEGMENT_LEN + 7] {
        for extra in [&b"x"[..], &[0u8; TAG_LEN][..], &[0u8; FULL_SEGMENT][..]] {
            let mut ct = encrypt(&data(len));
            ct.extend_from_slice(extra);
            assert_err(decrypt(&ct), "decrypt");
        }
    }
    // A second, independently valid stream appended.
    let mut ct = encrypt(&data(10));
    ct.extend_from_slice(&encrypt(&data(10)));
    assert_err(decrypt(&ct), "decrypt");
}

#[test]
fn debug_hides_secrets() {
    let secret = "hunter2-very-secret";
    let pass = Passphrase::new(secret.into());
    let pd = format!("{pass:?}");
    assert_eq!(pd, "Passphrase(***)");
    assert!(!pd.contains(secret));

    let k = derive_key(&pass, &Salt([3; SALT_LEN]), TEST_KDF).unwrap();
    let kd = format!("{k:?} {k:#?}");
    assert!(kd.contains("ArchiveKey(***)"));
    let key_bytes: &[u8; 32] = &k.0;
    let hex: String = key_bytes.iter().map(|b| format!("{b:02x}")).collect();
    assert!(!kd.contains(&hex));
    assert!(!kd.contains(&format!("{key_bytes:?}")));

    let w = EncryptWriter::new(Vec::new(), &k, &NONCE, AAD).unwrap();
    let r = DecryptReader::new(&b""[..], &k, &NONCE, AAD).unwrap();
    for d in [format!("{w:?}"), format!("{r:?}")] {
        assert!(
            !d.contains(&hex) && !d.contains(&format!("{key_bytes:?}")),
            "{d}"
        );
        assert!(!d.contains(secret));
    }
}

/// The only test using production KDF parameters (256 MiB); prints timing.
#[test]
fn default_kdf_params_work() {
    let start = std::time::Instant::now();
    let k = derive_key(
        &Passphrase::new("pw".into()),
        &Salt([9; SALT_LEN]),
        KdfParams::DEFAULT,
    )
    .unwrap();
    eprintln!("Argon2id DEFAULT derive took {:?}", start.elapsed());
    let ct = encrypt_with(&k, b"hello", AAD);
    assert_eq!(decrypt_with(&k, &ct, AAD).unwrap(), b"hello");
}
