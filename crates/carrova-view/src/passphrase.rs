//! The archive passphrase rule every front-end applies to user input
//! before it reaches the engine: a new archive needs a passphrase of at
//! least [`PassphraseUse::minimum`] strength (an archive file is
//! attackable offline); opening one takes whatever was typed.
//!
//! Strength is a rough guess-work estimate ([`passphrase_bits`]) mapped
//! to words through one table ([`STRENGTH_THRESHOLDS`]), shared with the
//! desktop meter through [`PassphrasePolicy`].

use std::collections::BTreeSet;
use std::fmt;

use carrova_core::Passphrase;
use serde::Serialize;

/// What the passphrase is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassphraseUse {
    /// Protecting a new archive: at least [`Self::minimum`] strength.
    NewArchive,
    /// Opening an existing archive: taken as typed.
    ExistingArchive,
}

impl PassphraseUse {
    /// The weakest passphrase accepted for this use.
    #[must_use]
    pub const fn minimum(self) -> PassphraseStrength {
        match self {
            Self::NewArchive => PassphraseStrength::Fair,
            Self::ExistingArchive => PassphraseStrength::Weak,
        }
    }
}

/// Strength in words, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PassphraseStrength {
    /// Below every threshold.
    Weak,
    /// Acceptable for a new archive.
    Fair,
    /// Comfortably strong.
    Good,
    /// Very strong.
    Strong,
}

impl fmt::Display for PassphraseStrength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Weak => "weak",
            Self::Fair => "fair",
            Self::Good => "good",
            Self::Strong => "strong",
        })
    }
}

/// The least [`passphrase_bits`] a strength needs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StrengthThreshold {
    /// The strength reached.
    pub strength: PassphraseStrength,
    /// Bits needed for it.
    pub min_bits: f64,
}

/// Strength by bits, weakest first; below the first entry is
/// [`PassphraseStrength::Weak`].
pub const STRENGTH_THRESHOLDS: [StrengthThreshold; 3] = [
    StrengthThreshold {
        strength: PassphraseStrength::Fair,
        min_bits: 50.0,
    },
    StrengthThreshold {
        strength: PassphraseStrength::Good,
        min_bits: 80.0,
    },
    StrengthThreshold {
        strength: PassphraseStrength::Strong,
        min_bits: 110.0,
    },
];

/// A class of characters and the alphabet size it adds when present.
struct CharClass {
    contains: fn(char) -> bool,
    alphabet: u32,
}

/// Every class [`passphrase_bits`] counts.
const CHAR_CLASSES: [CharClass; 4] = [
    CharClass {
        contains: |c| c.is_ascii_lowercase(),
        alphabet: 26,
    },
    CharClass {
        contains: |c| c.is_ascii_uppercase(),
        alphabet: 26,
    },
    CharClass {
        contains: |c| c.is_ascii_digit(),
        alphabet: 10,
    },
    CharClass {
        contains: |c| !c.is_ascii_alphanumeric(),
        alphabet: 33,
    },
];

/// Bits of guessing work, estimated from the length and the character
/// classes used; a character repeated more than twice adds nothing
/// (the effective length is at most twice the distinct characters).
#[must_use]
pub fn passphrase_bits(passphrase: &str) -> f64 {
    let alphabet: u32 = CHAR_CLASSES
        .iter()
        .filter(|class| passphrase.chars().any(class.contains))
        .map(|class| class.alphabet)
        .sum();
    if alphabet == 0 {
        return 0.0;
    }
    let distinct = passphrase.chars().collect::<BTreeSet<_>>().len();
    let effective = passphrase.chars().count().min(distinct * 2);
    let effective = u32::try_from(effective).unwrap_or(u32::MAX);
    f64::from(effective) * f64::from(alphabet).log2()
}

/// Strength in words of `passphrase` ([`STRENGTH_THRESHOLDS`]).
#[must_use]
pub fn strength(passphrase: &str) -> PassphraseStrength {
    let bits = passphrase_bits(passphrase);
    STRENGTH_THRESHOLDS
        .iter()
        .rev()
        .find(|t| bits >= t.min_bits)
        .map_or(PassphraseStrength::Weak, |t| t.strength)
}

/// The passphrase policy as data, for front-ends that show a live meter
/// (exported to the desktop as a constant).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PassphrasePolicy {
    /// [`STRENGTH_THRESHOLDS`].
    pub thresholds: Vec<StrengthThreshold>,
    /// [`PassphraseUse::minimum`] of a new archive.
    pub new_archive_minimum: PassphraseStrength,
}

impl PassphrasePolicy {
    /// This build's policy.
    #[must_use]
    pub fn current() -> Self {
        Self {
            thresholds: STRENGTH_THRESHOLDS.to_vec(),
            new_archive_minimum: PassphraseUse::NewArchive.minimum(),
        }
    }
}

/// Why a passphrase was refused for a new archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassphraseRejected {
    /// Nothing was typed.
    Empty,
    /// Weaker than the use allows.
    TooWeak {
        /// The passphrase's strength.
        strength: PassphraseStrength,
        /// The weakest one accepted.
        minimum: PassphraseStrength,
    },
}

impl fmt::Display for PassphraseRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("an archive needs a non-empty passphrase"),
            Self::TooWeak { strength, minimum } => write!(
                f,
                "the passphrase is {strength}; an archive needs at least a {minimum} one \
                 (anyone holding the file can try passphrases offline): use a longer one, \
                 e.g. several unrelated words"
            ),
        }
    }
}

impl std::error::Error for PassphraseRejected {}

/// `typed` wrapped as a [`Passphrase`] at once (the `String` moves into
/// the zeroizing secret), if it is acceptable for `purpose`.
///
/// # Errors
///
/// [`PassphraseRejected`] for an empty or too weak passphrase of a new
/// archive.
pub fn archive_passphrase(
    typed: String,
    purpose: PassphraseUse,
) -> Result<Passphrase, PassphraseRejected> {
    let verdict = check(&typed, purpose);
    let passphrase = Passphrase::new(typed);
    verdict.map(|()| passphrase)
}

/// Whether `typed` is acceptable for `purpose`.
fn check(typed: &str, purpose: PassphraseUse) -> Result<(), PassphraseRejected> {
    let minimum = purpose.minimum();
    if minimum == PassphraseStrength::Weak {
        return Ok(());
    }
    if typed.is_empty() {
        return Err(PassphraseRejected::Empty);
    }
    let strength = strength(typed);
    if strength < minimum {
        return Err(PassphraseRejected::TooWeak { strength, minimum });
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/passphrase/unit.rs"]
mod tests;
