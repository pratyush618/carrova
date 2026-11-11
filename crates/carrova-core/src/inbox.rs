//! The notification inbox: everything worth telling the user, kept for
//! [`RETENTION_DAYS`] days in `state_dir()/inbox.json` (`600`, replaced
//! atomically) whatever the user's pop-up preferences say.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use carrova_platform::Platform;
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, EncodeError, Result};
use crate::store::{ensure_dir, read_file, write_atomic};
use crate::time::{SECS_PER_DAY, now_unix};

/// File name of the inbox in the state directory.
pub const INBOX_FILE: &str = "inbox.json";
/// Days a notice is kept.
pub const RETENTION_DAYS: u64 = 30;

/// Identifies a [`Notice`] within the inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(transparent)]
pub struct NoticeId(pub u64);

/// What a [`Notice`] is about; the user's alert preferences are per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    /// A device wants to send something.
    SendRequest,
    /// A move finished.
    MoveFinished,
    /// A move failed.
    MoveFailed,
    /// A known device appeared nearby.
    DeviceNearby,
    /// An app update is ready to install.
    UpdateReady,
}

/// One entry of the bell list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Notice {
    /// Its identity.
    pub id: NoticeId,
    /// What it is about.
    pub kind: NoticeKind,
    /// When it happened (Unix seconds).
    pub at: u64,
    /// The user has seen it.
    pub read: bool,
    /// The other computer's name, when there is one.
    pub peer: Option<String>,
    /// One line saying what happened.
    pub summary: String,
}

/// Drop notices older than [`RETENTION_DAYS`] as of `now` (Unix seconds).
fn prune(notices: &mut Vec<Notice>, now: u64) {
    let oldest = now.saturating_sub(RETENTION_DAYS * SECS_PER_DAY);
    notices.retain(|n| n.at >= oldest);
}

/// The persisted notices. Cheap to share: clones see the same list.
#[derive(Debug, Clone)]
pub struct Inbox {
    platform: Arc<dyn Platform>,
    notices: Arc<Mutex<Vec<Notice>>>,
}

impl Inbox {
    /// The inbox of `platform`'s state directory, without notices older
    /// than [`RETENTION_DAYS`]. A file that cannot be parsed is renamed
    /// aside (`inbox.json.corrupt-<unix seconds>`) and the inbox starts
    /// empty; the second value is where it went, for the caller to tell
    /// the user.
    ///
    /// # Errors
    ///
    /// The file cannot be read, or cannot be set aside.
    pub fn open(platform: Arc<dyn Platform>) -> Result<(Self, Option<PathBuf>)> {
        Self::open_at(platform, now_unix())
    }

    /// [`Self::open`] as of `now` (Unix seconds).
    ///
    /// # Errors
    ///
    /// As [`Self::open`].
    pub fn open_at(platform: Arc<dyn Platform>, now: u64) -> Result<(Self, Option<PathBuf>)> {
        let path = file(&*platform)?;
        let mut set_aside = None;
        let mut notices: Vec<Notice> = match read_file(&*platform, &path)? {
            None => Vec::new(),
            Some(bytes) => serde_json::from_slice(&bytes).or_else(|_| {
                let aside = path.with_file_name(format!("{INBOX_FILE}.corrupt-{now}"));
                platform.rename_noreplace(&path, &aside)?;
                tracing::warn!(file = %aside.display(), "inbox was unreadable and was set aside");
                set_aside = Some(aside);
                Ok::<_, CoreError>(Vec::new())
            })?,
        };
        prune(&mut notices, now);
        let inbox = Self {
            platform,
            notices: Arc::new(Mutex::new(notices)),
        };
        Ok((inbox, set_aside))
    }

    /// Add an unread notice and save the inbox.
    ///
    /// # Errors
    ///
    /// The inbox cannot be written; the notice is not kept.
    pub fn push(
        &self,
        kind: NoticeKind,
        at: u64,
        peer: Option<String>,
        summary: String,
    ) -> Result<Notice> {
        let mut notices = self.lock();
        // Background mode runs for weeks: expire old notices here, not only on load.
        prune(&mut notices, at);
        let id = NoticeId(notices.iter().map(|n| n.id.0).max().map_or(0, |m| m + 1));
        let notice = Notice {
            id,
            kind,
            at,
            read: false,
            peer,
            summary,
        };
        notices.push(notice.clone());
        if let Err(e) = self.save(&notices) {
            notices.pop();
            return Err(e);
        }
        Ok(notice)
    }

    /// The notices, newest first.
    #[must_use]
    pub fn list(&self) -> Vec<Notice> {
        self.lock().iter().rev().cloned().collect()
    }

    /// Mark the notices `ids` as read.
    ///
    /// # Errors
    ///
    /// The inbox cannot be written.
    pub fn mark_read(&self, ids: &[NoticeId]) -> Result<()> {
        self.mark(|n| ids.contains(&n.id))
    }

    /// Mark every notice as read.
    ///
    /// # Errors
    ///
    /// The inbox cannot be written.
    pub fn mark_all_read(&self) -> Result<()> {
        self.mark(|_| true)
    }

    fn mark(&self, select: impl Fn(&Notice) -> bool) -> Result<()> {
        let mut notices = self.lock();
        for notice in notices.iter_mut().filter(|n| select(n)) {
            notice.read = true;
        }
        self.save(&notices)
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Notice>> {
        self.notices.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn save(&self, notices: &[Notice]) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(notices).map_err(|e| CoreError::Encode {
            what: INBOX_FILE.to_owned(),
            source: EncodeError::from(e),
        })?;
        let dir = self.platform.state_dir()?;
        ensure_dir(&*self.platform, &dir, 0o700)?;
        write_atomic(&*self.platform, &dir, INBOX_FILE, &bytes)
    }
}

fn file(platform: &dyn Platform) -> Result<PathBuf> {
    Ok(platform.state_dir()?.join(INBOX_FILE))
}
