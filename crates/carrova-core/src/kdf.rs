//! How hard a new archive's passphrase is to guess: the user's
//! [`KdfStrength`] choice, mapped to Argon2id parameters in exactly one
//! place ([`KdfStrength::params`]).
//!
//! Every strength is at least the engine's default, so no choice weakens
//! an archive below [`KdfParams::DEFAULT`] (and so never below the
//! writer floor [`KdfParams::FLOOR`]); stronger choices cost more memory
//! and time when the archive is written *and* every time it is opened.

use carrova_archive::WriteOptions;
use carrova_crypto::KdfParams;
use serde::{Deserialize, Serialize};

/// Passphrase key-derivation strength of a new archive. The default is
/// the suggestion; the user may pick a stronger one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum KdfStrength {
    /// Suggested: 256 MiB, 3 passes ([`KdfParams::DEFAULT`]); about a
    /// second on a typical laptop.
    #[default]
    Standard,
    /// 512 MiB, 4 passes: roughly 2–3× the time and twice the memory.
    Strong,
    /// 1 GiB, 6 passes: roughly 6–8× the time and four times the memory;
    /// machines with little free RAM may struggle to open the archive.
    Maximum,
}

impl KdfStrength {
    /// Every strength, weakest first.
    pub const ALL: [Self; 3] = [Self::Standard, Self::Strong, Self::Maximum];

    /// The Argon2id parameters of this strength. Each passes
    /// [`KdfParams::validate_for_write`] and is at least
    /// [`KdfParams::DEFAULT`].
    #[must_use]
    pub const fn params(self) -> KdfParams {
        match self {
            Self::Standard => KdfParams::DEFAULT,
            Self::Strong => KdfParams {
                m_kib: 512 * 1024,
                t: 4,
                p: 1,
            },
            Self::Maximum => KdfParams {
                m_kib: 1024 * 1024,
                t: 6,
                p: 1,
            },
        }
    }

    /// Stable lowercase name (the serde spelling), e.g. `strong`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Strong => "strong",
            Self::Maximum => "maximum",
        }
    }
}

impl From<KdfStrength> for WriteOptions {
    fn from(strength: KdfStrength) -> Self {
        Self {
            kdf: strength.params(),
        }
    }
}

#[cfg(test)]
#[path = "../tests/kdf/unit.rs"]
mod tests;
