//! The user's opt-in choice, persisted next to the reports.

use std::fs;

use serde::{Deserialize, Serialize};

use crate::error::ReportError;
use crate::store::ReportStore;

const CONSENT_FILE: &str = "consent.json";

/// Whether the user allows Carrova to keep local error reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ReportConsent {
    /// Not asked yet (front-ends ask on first run). Nothing is recorded.
    #[default]
    Unset,
    /// Reports are written locally (never sent without a user action).
    Allowed,
    /// Nothing is recorded.
    Denied,
}

impl ReportConsent {
    /// Only an explicit "allow" permits writing reports.
    #[must_use]
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }
}

#[derive(Serialize, Deserialize)]
struct ConsentFile {
    consent: ReportConsent,
}

impl ReportStore {
    /// The persisted choice; [`ReportConsent::Unset`] if never set.
    ///
    /// # Errors
    ///
    /// [`ReportError::Io`] or [`ReportError::Malformed`] if the file exists
    /// but cannot be read.
    pub fn consent(&self) -> Result<ReportConsent, ReportError> {
        let path = self.dir().join(CONSENT_FILE);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<ConsentFile>(&bytes)
                .map(|file| file.consent)
                .map_err(ReportError::malformed(&path)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(ReportConsent::Unset),
            Err(err) => Err(ReportError::io(&path)(err)),
        }
    }

    /// Persists the choice. Denying also deletes every stored report.
    ///
    /// # Errors
    ///
    /// [`ReportError::Io`] if the file cannot be written or reports removed.
    pub fn set_consent(&self, consent: ReportConsent) -> Result<(), ReportError> {
        let json = serde_json::to_vec(&ConsentFile { consent })
            .map_err(ReportError::malformed(self.dir().join(CONSENT_FILE)))?;
        self.write_file(CONSENT_FILE, &json)?;
        if consent == ReportConsent::Denied {
            self.clear()?;
        }
        Ok(())
    }
}
