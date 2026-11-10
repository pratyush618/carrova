//! `BundleIndex` parsing: format errors take precedence over schema errors.

use carrova_bundle::{BundleError, BundleIndex};

#[test]
fn future_format_is_reported_before_unknown_fields() {
    let err = BundleIndex::parse(br#"{"format":4,"shiny":true}"#).unwrap_err();
    assert!(
        matches!(err, BundleError::UnsupportedFormat { found: 4, .. }),
        "{err}"
    );
}
