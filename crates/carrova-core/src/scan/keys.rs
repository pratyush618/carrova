//! Key files an item carries ([`KeyInfo`]), so the user can tell which
//! key is which before choosing to move it.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;

use carrova_platform::{FileKind, Platform, SemanticRoot};
use carrova_recipe::FileEntry;
use data_encoding::{BASE64, BASE64_NOPAD};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::key::RootedPath;

/// A key file found in an item (SSH public/private key pairs today).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KeyInfo {
    /// The key file, as `root:rel`.
    pub path: String,
    /// Key algorithm.
    pub kind: KeyKind,
    /// `SHA256:<base64>` fingerprint of the public key, as `ssh-keygen -l`
    /// prints it. Never the key itself.
    pub fingerprint: String,
    /// The key's comment (often `user@host`), if it has one.
    pub comment: Option<String>,
}

/// Algorithm of a [`KeyInfo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum KeyKind {
    /// `ssh-ed25519`.
    Ed25519,
    /// `ecdsa-sha2-*`.
    Ecdsa,
    /// `ssh-rsa`.
    Rsa,
    /// Any other algorithm.
    Other,
}

impl KeyKind {
    /// The algorithm an OpenSSH key type string (`ssh-ed25519`, …) names.
    #[must_use]
    pub fn of(key_type: &str) -> Self {
        match key_type {
            "ssh-ed25519" => Self::Ed25519,
            "ssh-rsa" => Self::Rsa,
            t if t.starts_with("ecdsa-sha2-") => Self::Ecdsa,
            _ => Self::Other,
        }
    }
}

/// What an OpenSSH public key line (`type base64 [comment]`) tells.
#[derive(Debug, PartialEq, Eq)]
struct PublicKey {
    kind: KeyKind,
    fingerprint: String,
    comment: Option<String>,
}

impl PublicKey {
    /// The first line of a `.pub` file, `None` if it is not a public key.
    fn parse(text: &str) -> Option<Self> {
        let mut parts = text.lines().next()?.split_whitespace();
        let kind = KeyKind::of(parts.next()?);
        let blob = BASE64.decode(parts.next()?.as_bytes()).ok()?;
        let comment = parts.collect::<Vec<_>>().join(" ");
        Some(Self {
            kind,
            // As `ssh-keygen -l`: unpadded base64 of the blob's SHA-256.
            fingerprint: format!("SHA256:{}", BASE64_NOPAD.encode(&Sha256::digest(&blob))),
            comment: (!comment.is_empty()).then_some(comment),
        })
    }
}

/// Longest `.pub` file read; real ones are well under 1 KiB.
const MAX_PUB_LEN: u64 = 16 * 1024;

/// SSH keys among an item's `entries` (under `roots`): one per readable
/// `<name>.pub`, reported at the private key `<name>` when the item has
/// it. Unreadable or unparseable `.pub` files are skipped.
pub(super) fn find(
    entries: &[FileEntry],
    roots: &BTreeMap<SemanticRoot, PathBuf>,
    platform: &dyn Platform,
) -> Vec<KeyInfo> {
    let file = |root: SemanticRoot, rel: &str| {
        entries
            .iter()
            .any(|e| e.kind == FileKind::File && e.root == root && e.rel == rel)
    };
    entries
        .iter()
        .filter(|e| e.kind == FileKind::File)
        .filter_map(|e| {
            let private = e.rel.strip_suffix(".pub")?;
            let mut abs = roots.get(&e.root)?.clone();
            abs.extend(e.rel.split('/'));
            let mut text = String::new();
            platform
                .open_read_nofollow(&abs)
                .ok()?
                .take(MAX_PUB_LEN)
                .read_to_string(&mut text)
                .ok()?;
            let key = PublicKey::parse(&text)?;
            let rel = if file(e.root, private) {
                private
            } else {
                &e.rel
            };
            Some(KeyInfo {
                path: RootedPath::new(e.root, rel).to_string(),
                kind: key.kind,
                fingerprint: key.fingerprint,
                comment: key.comment,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../../tests/scan/keys/unit.rs"]
mod tests;
