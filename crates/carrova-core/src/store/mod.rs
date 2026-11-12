//! Session persistence under `Platform::state_dir()/sessions/<id>/`:
//! `session.json` (atomic replace), `plan.json`, `selection.json`,
//! `journal.cbor`, `staging/`, `backup/`, `log.jsonl`. Dirs `700`, files `600`.
//! A session resumes from its last persisted state after a restart.
//!
//! JSON documents are written atomically: a sibling temp file
//! (`.<name>.tmp`, create-new, `600`) is written and fsynced, renamed over
//! the old document, then the directory is fsynced. Session ids are
//! validated with [`SessionId::is_valid`] before they are joined into a path.
//!
//! No locking: one process drives a session at a time.

mod files;
mod record;
mod resume_key;
pub(crate) mod secret_file;

use std::path::{Path, PathBuf};

use carrova_platform::{FileKind, Platform};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) use files::{ensure_dir, read_file, write_atomic};
pub use record::{ArchiveFingerprint, CorruptSession, SessionRecord, Source};
pub use resume_key::{read_resume_secret, wipe_resume_secret, write_resume_secret};

use crate::error::{CoreError, Corruption, EncodeError, Result};
use crate::fsx;
use crate::session::{Role, SessionId, State};
use crate::time;

/// File name of the session record.
pub const SESSION_FILE: &str = "session.json";
/// File name of the persisted plan (import).
pub const PLAN_FILE: &str = "plan.json";
/// File name of the persisted selection (export).
pub const SELECTION_FILE: &str = "selection.json";
/// File name of the per-session event log.
pub const LOG_FILE: &str = "log.jsonl";
/// Staging directory name inside a session dir.
pub const STAGING_DIR: &str = "staging";
/// LAN resume secret inside a session dir (`600`, raw bytes).
pub const RESUME_KEY_FILE: &str = "resume.key";

/// Session ids tried before [`Store::create`] gives up.
const ID_ATTEMPTS: usize = 16;

/// Handle to the session store.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Open (create if needed, mode 700) `state_dir()/sessions`.
    pub fn open(platform: &dyn Platform) -> Result<Self> {
        let root = platform.state_dir()?.join("sessions");
        ensure_dir(platform, &root, 0o700)?;
        Ok(Self { root })
    }

    /// `state_dir()/sessions`.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory of one session (id validated before joining).
    pub fn session_dir(&self, id: &SessionId) -> Result<PathBuf> {
        if !SessionId::is_valid(&id.0) {
            return Err(CoreError::InvalidSessionId(id.0.clone()));
        }
        Ok(self.root.join(&id.0))
    }

    /// Create a new session (fresh id, dir `700`, `session.json`).
    pub fn create(
        &self,
        platform: &dyn Platform,
        role: Role,
        state: State,
        source: Option<Source>,
    ) -> Result<SessionRecord> {
        let now = time::now_unix();
        for _ in 0..ID_ATTEMPTS {
            let id = SessionId(format!("{}-{}", time::timestamp(now), random_hex6()?));
            match platform.create_dir(&self.session_dir(&id)?, Some(0o700)) {
                Ok(()) => {}
                Err(e) => {
                    let e = CoreError::from(e);
                    if fsx::kind_of(&e) == Some(std::io::ErrorKind::AlreadyExists) {
                        continue;
                    }
                    return Err(e);
                }
            }
            let record = SessionRecord {
                id,
                role,
                state,
                created_at: now,
                updated_at: now,
                source,
                approved: false,
                checklist: None,
                error: None,
                bytes_moved: None,
                peer_os: None,
                expires_at: None,
            };
            self.save(platform, &record)?;
            return Ok(record);
        }
        Err(CoreError::IdsExhausted)
    }

    /// Persist `record` atomically. Once the record is terminal
    /// (`Done`, `RolledBack`, `Failed`), the session's LAN resume secret
    /// is wiped too (best effort after the record is saved: a failure is
    /// logged, the state change stands).
    pub fn save(&self, platform: &dyn Platform, record: &SessionRecord) -> Result<()> {
        self.write_json(platform, &record.id, SESSION_FILE, record)?;
        if record.state.is_terminal()
            && let Err(e) = wipe_resume_secret(platform, &self.session_dir(&record.id)?)
        {
            tracing::warn!(session = %record.id, error = %e, "could not wipe resume secret");
        }
        Ok(())
    }

    /// Atomically write a JSON document into a session dir.
    pub fn write_json<T: Serialize>(
        &self,
        platform: &dyn Platform,
        id: &SessionId,
        name: &str,
        value: &T,
    ) -> Result<()> {
        let dir = self.session_dir(id)?;
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| CoreError::Encode {
            what: name.to_owned(),
            source: EncodeError::from(e),
        })?;
        write_atomic(platform, &dir, name, &bytes)
    }

    /// Read a JSON document from a session dir (`None` if absent).
    pub fn read_json<T: DeserializeOwned>(
        &self,
        platform: &dyn Platform,
        id: &SessionId,
        name: &str,
    ) -> Result<Option<T>> {
        let path = self.session_dir(id)?.join(name);
        match read_file(platform, &path)? {
            None => Ok(None),
            Some(bytes) => {
                serde_json::from_slice(&bytes)
                    .map(Some)
                    .map_err(|e| CoreError::CorruptFile {
                        file: format!("{}/{name}", id.0),
                        reason: Corruption::from(e),
                    })
            }
        }
    }

    /// All sessions, newest first. Unreadable records are skipped (see
    /// [`Self::list_detailed`] to get them).
    pub fn list(&self, platform: &dyn Platform) -> Result<Vec<SessionRecord>> {
        Ok(self.list_detailed(platform)?.0)
    }

    /// All sessions newest first, plus records that could not be read.
    pub fn list_detailed(
        &self,
        platform: &dyn Platform,
    ) -> Result<(Vec<SessionRecord>, Vec<CorruptSession>)> {
        let mut names: Vec<String> = platform
            .read_dir(&self.root)?
            .into_iter()
            .filter(|e| e.kind == FileKind::Dir)
            .map(|e| e.name.to_string_lossy().into_owned())
            .filter(|n| SessionId::is_valid(n))
            .collect();
        names.sort_unstable_by(|a, b| b.cmp(a));
        let mut records = Vec::new();
        let mut corrupt = Vec::new();
        for name in names {
            let id = SessionId(name.clone());
            match self.load(platform, &id) {
                Ok(r) if r.id == id => records.push(r),
                Ok(_) => corrupt.push(CorruptSession {
                    id: name,
                    reason: "record id does not match its directory".into(),
                }),
                Err(e) => corrupt.push(CorruptSession {
                    id: name,
                    reason: e.to_string(),
                }),
            }
        }
        records.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        Ok((records, corrupt))
    }

    /// Load one session.
    pub fn load(&self, platform: &dyn Platform, id: &SessionId) -> Result<SessionRecord> {
        self.read_json(platform, id, SESSION_FILE)?
            .ok_or_else(|| CoreError::NoSuchSession(id.clone()))
    }
}

/// Six random hex digits (the unique tail of a session id).
fn random_hex6() -> Result<String> {
    let mut b = [0u8; 3];
    getrandom::fill(&mut b).map_err(|_| CoreError::Random)?;
    Ok(data_encoding::HEXLOWER.encode(&b))
}
