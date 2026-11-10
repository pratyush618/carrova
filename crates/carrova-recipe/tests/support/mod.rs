//! Helpers shared by the integration test binaries.

#![allow(clippy::unwrap_used)]

use std::path::Path;

pub(crate) fn symlink(target: impl AsRef<Path>, link: &Path) {
    carrova_platform::fake::symlink(target.as_ref(), link).unwrap();
}
