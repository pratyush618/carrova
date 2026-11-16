//! How far back the phone's texts reach its linked devices ("Keep in
//! sync"), chosen on the phone. Stored in `<state>/phone/messages.json`
//! (`600`, atomic).

use carrova_platform::Platform;
use serde::{Deserialize, Serialize};

use super::apps::dir;
use crate::error::{CoreError, Corruption, EncodeError, Result};
use crate::store::{read_file, write_atomic};

const FILE: &str = "messages.json";
const VERSION: u16 = 1;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// How far back SMS pages go.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SyncWindow {
    /// The last 30 days.
    #[default]
    Days30,
    /// The last 365 days.
    Year,
    /// Everything the phone holds.
    All,
}

#[derive(Serialize, Deserialize)]
struct Saved {
    version: u16,
    keep: SyncWindow,
}

impl SyncWindow {
    /// The saved choice; the default if none was saved.
    ///
    /// # Errors
    /// The file cannot be read, or is corrupt or from a newer version.
    pub fn open(platform: &dyn Platform) -> Result<Self> {
        let Some(bytes) = read_file(platform, &dir(platform)?.join(FILE))? else {
            return Ok(Self::default());
        };
        let corrupt = |reason| CoreError::CorruptFile {
            file: FILE.to_owned(),
            reason,
        };
        let saved: Saved = serde_json::from_slice(&bytes).map_err(|e| corrupt(e.into()))?;
        if saved.version > VERSION {
            return Err(corrupt(Corruption::Version(saved.version)));
        }
        Ok(saved.keep)
    }

    /// Save `self` as the choice.
    ///
    /// # Errors
    /// The file cannot be written; the choice is then not kept.
    pub fn save(self, platform: &dyn Platform) -> Result<()> {
        let saved = Saved {
            version: VERSION,
            keep: self,
        };
        let bytes = serde_json::to_vec_pretty(&saved).map_err(|e| CoreError::Encode {
            what: "messages sync window".to_owned(),
            source: EncodeError::from(e),
        })?;
        write_atomic(platform, &dir(platform)?, FILE, &bytes)
    }

    /// The oldest time pages reach back to at `now_ms`; `None`: no limit.
    #[must_use]
    pub fn since_ms(self, now_ms: i64) -> Option<i64> {
        let days = match self {
            Self::Days30 => 30,
            Self::Year => 365,
            Self::All => return None,
        };
        Some(now_ms.saturating_sub(days * DAY_MS))
    }
}
