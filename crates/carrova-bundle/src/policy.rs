//! Update admission policy, the one place it lives: which offered bundle
//! may replace the installed one.
//!
//! An offered index is compared with the [`Baseline`] (the verified
//! installed bundle plus the versions the user rolled back):
//!
//! * same version → nothing to do;
//! * a version the user rolled back → never offered again;
//! * older → refused (rollback protection);
//! * more than [`MAX_VERSION_JUMP`] ahead → refused, so a leaked signer key
//!   cannot push a version so high that no legitimate release can follow;
//! * created before the installed bundle → refused (a newer version is
//!   never older in time).
//!
//! With nothing installed, any version not rolled back is admitted.

use std::collections::BTreeSet;

use crate::{BundleError, BundleIndex, BundleVersion, Result};

/// Largest version step an update may take over the installed bundle.
pub const MAX_VERSION_JUMP: u64 = 1000;

/// What an offered bundle is compared with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Baseline {
    /// Version and creation time of the verified installed bundle.
    pub(crate) installed: Option<Installed>,
    /// Versions the user rolled back.
    pub(crate) refused: BTreeSet<BundleVersion>,
}

/// The facts of the installed bundle the policy needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Installed {
    /// Installed version.
    pub(crate) version: BundleVersion,
    /// Its creation time (unix seconds).
    pub(crate) created: u64,
}

impl Installed {
    /// The facts of `index`.
    pub(crate) fn of(index: &BundleIndex) -> Self {
        Self {
            version: index.version,
            created: index.created,
        }
    }
}

/// Outcome of [`Baseline::admit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Admission {
    /// Install it.
    Newer,
    /// It is the installed version: nothing to do.
    Current,
    /// The user rolled it back: do not offer it.
    Refused,
}

impl Baseline {
    /// Judge `offered` (see the module docs for the rules and their order).
    pub(crate) fn admit(&self, offered: &BundleIndex) -> Result<Admission> {
        let version = offered.version;
        if self.installed.is_some_and(|i| i.version == version) {
            return Ok(Admission::Current);
        }
        if self.refused.contains(&version) {
            return Ok(Admission::Refused);
        }
        let Some(installed) = self.installed else {
            return Ok(Admission::Newer);
        };
        if version < installed.version {
            return Err(BundleError::Rollback {
                offered: version,
                installed: installed.version,
            });
        }
        if version.0 - installed.version.0 > MAX_VERSION_JUMP {
            return Err(BundleError::VersionJump {
                offered: version,
                installed: installed.version,
                max: MAX_VERSION_JUMP,
            });
        }
        if offered.created < installed.created {
            return Err(BundleError::CreatedBackwards {
                offered: offered.created,
                installed: installed.created,
            });
        }
        Ok(Admission::Newer)
    }
}
