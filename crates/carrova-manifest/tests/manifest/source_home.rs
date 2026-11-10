//! `source_home`: the sender's home folder, for plugins that rewrite
//! absolute paths. Optional and omitted when unknown.

use carrova_manifest::{Manifest, ManifestError};

use super::sample;

#[test]
fn source_home_round_trips_and_is_omitted_when_unknown() {
    let mut m = sample();
    assert_eq!(m.source_home, None);
    let without = m.to_cbor_vec().unwrap();
    assert!(!String::from_utf8_lossy(&without).contains("source_home"));

    m.source_home = Some("/home/ada".into());
    let back = Manifest::from_cbor(m.to_cbor_vec().unwrap().as_slice()).unwrap();
    assert_eq!(back.source_home.as_deref(), Some("/home/ada"));
}

#[test]
fn a_source_home_with_control_characters_is_refused() {
    let mut m = sample();
    m.source_home = Some("/home/a\ntarget_home=/".into());
    assert!(matches!(
        m.validate(),
        Err(ManifestError::UnsafeText {
            field: "source_home",
            ..
        })
    ));
}
