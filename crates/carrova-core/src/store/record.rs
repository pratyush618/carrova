//! The persisted session record (`session.json`) and its parts.

use std::path::{Path, PathBuf};

use carrova_crypto::pairing::CertHash;
use carrova_manifest::Blake3Hash;
use serde::{Deserialize, Serialize};

use crate::checklist::Checklist;
use crate::session::{Role, SessionId, State};

/// Persisted session record (`session.json`).
///
/// Records written by Phase 2 (`archive` + `archive_fingerprint` fields
/// instead of `source`) still load: they become [`Source::Archive`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(from = "RecordOnDisk")]
pub struct SessionRecord {
    pub id: SessionId,
    pub role: Role,
    pub state: State,
    /// Unix seconds.
    pub created_at: u64,
    pub updated_at: u64,
    /// Where the data comes from (import) or goes to (export). `None`
    /// until known: an export before its target is chosen.
    pub source: Option<Source>,
    /// Plan approved by the user (import only).
    pub approved: bool,
    pub checklist: Option<Checklist>,
    /// Last error message, if Failed.
    pub error: Option<String>,
    /// Bytes this machine moved once the session finished: archive file
    /// size (archive export), chunk bytes served (LAN send), bytes
    /// written into the user's roots (import). `None` until then, and for
    /// records written before Carrova tracked it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_moved: Option<u64>,
}

impl SessionRecord {
    /// Archive path, for archive sessions.
    #[must_use]
    pub fn archive_path(&self) -> Option<&Path> {
        match &self.source {
            Some(Source::Archive { path, .. }) => Some(path),
            Some(Source::Lan { .. }) | None => None,
        }
    }

    /// Fingerprint of the archive opened for planning (archive imports).
    #[must_use]
    pub fn archive_fingerprint(&self) -> Option<&ArchiveFingerprint> {
        match &self.source {
            Some(Source::Archive { fingerprint, .. }) => fingerprint.as_ref(),
            Some(Source::Lan { .. }) | None => None,
        }
    }
}

/// The other end of a session: an archive file or a LAN peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// Archive file: the export target or the import source.
    Archive {
        /// Absolute archive path, for display and resume.
        path: PathBuf,
        /// Import: digests of the archive as opened for planning; checked
        /// every time the archive is reopened (staging, resume). `None` for
        /// exports.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fingerprint: Option<ArchiveFingerprint>,
    },
    /// LAN peer (Phase 3).
    Lan {
        /// SHA-256 of the peer's certificate on the latest connection;
        /// `None` on the sender until a receiver has paired. Certificates
        /// are per process, so this changes after either side restarts.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(as = "Option<String>"))]
        peer_cert_hash: Option<CertHash>,
        /// BLAKE3 of the manifest CBOR as sent (sender) or received
        /// (receiver); a resumed session must see the same manifest.
        #[cfg_attr(feature = "ts", ts(as = "String"))]
        manifest_fingerprint: Blake3Hash,
        /// The peer's device name as it announced it (sanitised), if
        /// known: the receiver's name on the sender (`None` until it is
        /// known), the sender's manifest `source.device_name` on the
        /// receiver.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        peer_name: Option<String>,
    },
}

/// On-disk form of [`SessionRecord`], accepting the Phase 2 layout.
#[derive(Deserialize)]
struct RecordOnDisk {
    id: SessionId,
    role: Role,
    state: State,
    created_at: u64,
    updated_at: u64,
    #[serde(default)]
    source: Option<Source>,
    /// Phase 2: archive path (now [`Source::Archive::path`]).
    #[serde(default)]
    archive: Option<PathBuf>,
    /// Phase 2: archive fingerprint (now [`Source::Archive::fingerprint`]).
    #[serde(default)]
    archive_fingerprint: Option<ArchiveFingerprint>,
    approved: bool,
    checklist: Option<Checklist>,
    error: Option<String>,
    #[serde(default)]
    bytes_moved: Option<u64>,
}

impl From<RecordOnDisk> for SessionRecord {
    fn from(r: RecordOnDisk) -> Self {
        let legacy = r.archive.map(|path| Source::Archive {
            path,
            fingerprint: r.archive_fingerprint,
        });
        Self {
            id: r.id,
            role: r.role,
            state: r.state,
            created_at: r.created_at,
            updated_at: r.updated_at,
            source: r.source.or(legacy),
            approved: r.approved,
            checklist: r.checklist,
            error: r.error,
            bytes_moved: r.bytes_moved,
        }
    }
}

/// Identity of an opened archive: BLAKE3 of the plaintext header bytes and
/// of the (re-encoded) manifest CBOR. A swapped or re-encrypted archive
/// changes the header (fresh salt/nonce); a different manifest changes the
/// second digest; chunk data is verified against the manifest anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArchiveFingerprint {
    /// BLAKE3 of the plaintext header bytes (hex).
    #[cfg_attr(feature = "ts", ts(as = "String"))]
    pub header: Blake3Hash,
    /// BLAKE3 of the re-encoded manifest CBOR (hex).
    #[cfg_attr(feature = "ts", ts(as = "String"))]
    pub manifest: Blake3Hash,
}

/// A `session.json` that could not be read (reported, never fatal).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CorruptSession {
    /// Directory name under `sessions/`.
    pub id: String,
    pub reason: String,
}

#[cfg(test)]
#[path = "../../tests/store/record/unit.rs"]
mod tests;
