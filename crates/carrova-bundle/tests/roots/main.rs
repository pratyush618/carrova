//! Trust roots: the embedded release roots parse and malformed root files are rejected.

use carrova_bundle::{parse_roots, release_roots};

#[test]
fn embedded_roots_parse() {
    assert!(release_roots().unwrap().iter().count() >= 1);
}

#[test]
fn empty_or_unknown_shapes_are_rejected() {
    assert!(parse_roots(r#"{"roots":[]}"#).is_err());
    assert!(parse_roots(r#"{"roots":["00"]}"#).is_err());
    assert!(parse_roots(r#"["x"]"#).is_err());
}
