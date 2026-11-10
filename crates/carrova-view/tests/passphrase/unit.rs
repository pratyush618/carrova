use super::*;

#[test]
fn a_new_archive_refuses_an_empty_passphrase() {
    assert_eq!(
        archive_passphrase(String::new(), PassphraseUse::NewArchive).err(),
        Some(PassphraseRejected::Empty)
    );
}

#[test]
fn a_new_archive_refuses_a_weak_passphrase() {
    assert_eq!(
        archive_passphrase("hunter2".into(), PassphraseUse::NewArchive).err(),
        Some(PassphraseRejected::TooWeak {
            strength: PassphraseStrength::Weak,
            minimum: PassphraseStrength::Fair,
        })
    );
    assert!(archive_passphrase("orbit lemon".into(), PassphraseUse::NewArchive).is_ok());
}

#[test]
fn opening_an_archive_takes_the_passphrase_as_typed() {
    assert!(archive_passphrase(String::new(), PassphraseUse::ExistingArchive).is_ok());
    assert!(archive_passphrase("x".into(), PassphraseUse::ExistingArchive).is_ok());
}

#[test]
fn short_or_repetitive_passphrases_are_weak() {
    assert_eq!(strength(""), PassphraseStrength::Weak);
    assert_eq!(strength("hunter2"), PassphraseStrength::Weak);
    assert_eq!(
        strength("aaaaaaaaaaaaaaaaaaaaaaaa"),
        PassphraseStrength::Weak
    );
}

#[test]
fn more_unrelated_words_are_stronger() {
    assert_eq!(strength("orbit lemon"), PassphraseStrength::Fair);
    assert_eq!(strength("orbit lemon tiger"), PassphraseStrength::Good);
    assert_eq!(
        strength("orbit lemon tiger canvas"),
        PassphraseStrength::Strong
    );
}

#[test]
fn bits_follow_length_classes_and_repeats() {
    assert!(passphrase_bits("").abs() < f64::EPSILON);
    // 4 distinct lowercase letters, length 8: min(8, 2·4) · log2(26).
    let expected = 8.0 * 26f64.log2();
    assert!((passphrase_bits("abcdabcd") - expected).abs() < 1e-9);
    // Repeats count once: 1 distinct letter caps the length at 2.
    assert!((passphrase_bits("aaaaaaaa") - 2.0 * 26f64.log2()).abs() < 1e-9);
}

#[test]
fn the_policy_table_is_ordered_and_names_the_new_archive_minimum() {
    let policy = PassphrasePolicy::current();
    assert_eq!(policy.new_archive_minimum, PassphraseStrength::Fair);
    let bits: Vec<f64> = policy.thresholds.iter().map(|t| t.min_bits).collect();
    assert!(bits.windows(2).all(|w| w[0] < w[1]), "{bits:?}");
}
