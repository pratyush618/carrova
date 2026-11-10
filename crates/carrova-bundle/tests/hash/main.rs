//! Content hashes and asset names: strict hex parsing, blob naming and exact-byte matching.

use carrova_bundle::{Asset, BlobHash, BlobKind, BlobRef, BundleError};

#[test]
fn hash_text_is_strict_lowercase_hex() {
    let h = BlobHash::of(b"recipe");
    let text = h.to_string();
    assert_eq!(text.parse::<BlobHash>().unwrap(), h);
    for bad in [
        text.to_uppercase(),
        text[..63].to_owned(),
        format!("{text}0"),
        format!("../{}", &text[3..]),
        format!("{}/x", &text[..62]),
        String::new(),
    ] {
        assert!(
            matches!(bad.parse::<BlobHash>(), Err(BundleError::MalformedHash(_))),
            "{bad}"
        );
    }
}

#[test]
fn blob_names_are_hash_plus_kind_extension() {
    let h = BlobHash::of(b"x");
    assert_eq!(
        Asset::Blob(BlobKind::Recipe, h).file_name(),
        format!("{h}.yaml")
    );
    assert_eq!(
        Asset::Blob(BlobKind::Detect, h).file_name(),
        format!("{h}.json")
    );
    assert_eq!(Asset::Index.file_name(), "index.json");
}

#[test]
fn blob_ref_matches_only_exact_bytes() {
    let r = BlobRef::of(b"abc");
    assert!(r.matches(b"abc"));
    assert!(!r.matches(b"abd"));
    assert!(!BlobRef { size: 4, ..r }.matches(b"abc"));
}
