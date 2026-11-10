//! Serde adapter storing [`OsKind`] as its stable id ([`OsKind::as_str`]),
//! the lowercase string format 1 and 2 manifests already carry.

use carrova_platform::OsKind;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serializer};

pub(crate) fn serialize<S: Serializer>(os: &OsKind, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(os.as_str())
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<OsKind, D::Error> {
    String::deserialize(deserializer)?
        .parse()
        .map_err(D::Error::custom)
}
