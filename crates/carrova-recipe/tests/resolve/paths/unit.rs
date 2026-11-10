//! Private lexical normalisation (wired from `src/resolve/paths.rs`).

use std::path::Path;

use super::*;

#[test]
fn normalize_lexically() {
    assert_eq!(normalize(Path::new("/a/./b/../c")), Path::new("/a/c"));
    assert_eq!(normalize(Path::new("/a/../../..")), Path::new("/"));
    assert_eq!(normalize(Path::new("a/../../b")), Path::new("../b"));
}
