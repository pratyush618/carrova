use super::*;

#[test]
fn every_strength_is_writable_and_never_weaker_than_the_default() {
    for strength in KdfStrength::ALL {
        let p = strength.params();
        assert!(p.validate_for_write().is_ok(), "{strength:?}");
        assert!(p.m_kib >= KdfParams::DEFAULT.m_kib, "{strength:?}");
        assert!(p.t >= KdfParams::DEFAULT.t, "{strength:?}");
    }
}

#[test]
fn the_suggested_strength_is_the_engine_default() {
    assert_eq!(KdfStrength::default(), KdfStrength::Standard);
    assert_eq!(KdfStrength::Standard.params(), KdfParams::DEFAULT);
    assert_eq!(
        WriteOptions::from(KdfStrength::default()).kdf,
        WriteOptions::default().kdf
    );
}

#[test]
fn stronger_choices_cost_strictly_more() {
    for pair in KdfStrength::ALL.windows(2) {
        let (weaker, stronger) = (pair[0].params(), pair[1].params());
        assert!(stronger.m_kib > weaker.m_kib, "{pair:?}");
        assert!(stronger.t >= weaker.t, "{pair:?}");
    }
}

#[test]
fn names_are_the_serde_spelling() {
    for strength in KdfStrength::ALL {
        assert_eq!(
            serde_json::to_string(&strength).unwrap(),
            format!("\"{}\"", strength.as_str())
        );
    }
}
