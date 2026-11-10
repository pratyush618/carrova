//! Pairing codes: the public [`Nameplate`] and the secret words.

use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};
use std::str::FromStr;

use spake2::Password;
use zeroize::{Zeroize, Zeroizing};

use super::wordlist::{WORDLIST_LEN, WORDS};
use super::{
    CODE_SEPARATOR, CODE_WORDS, CodeError, MIN_WORD_PREFIX, NAMEPLATES, PASSWORD_LABEL,
    PASSWORD_LEN, PairingError, Result,
};

/// Public part of a pairing code (`1..=999`): selects the sender on the LAN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Nameplate(u16);

impl Nameplate {
    /// Nameplate `n`, if inside [`NAMEPLATES`].
    pub fn new(n: u16) -> Result<Self> {
        if NAMEPLATES.contains(&n) {
            Ok(Self(n))
        } else {
            Err(CodeError::Nameplate.into())
        }
    }

    /// Numeric value.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for Nameplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Nameplate {
    type Err = PairingError;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(CodeError::Nameplate.into());
        }
        Self::new(s.parse().map_err(|_| CodeError::Nameplate)?)
    }
}

/// Index of a word in the word list. Invariant: `< WORDLIST_LEN` (only
/// built by [`Word::uniform`] and [`Word::parse`]).
#[derive(Clone, Copy, PartialEq, Eq, Zeroize)]
struct Word(u16);

impl Word {
    /// Uniformly random word.
    fn uniform() -> Result<Self> {
        uniform_below(WORD_COUNT).map(Self)
    }

    /// Look up one typed word: case-insensitive, either the whole word or
    /// an unambiguous prefix of at least [`MIN_WORD_PREFIX`] letters.
    fn parse(typed: &str) -> Option<Self> {
        let typed = typed.as_bytes();
        if typed.len() < MIN_WORD_PREFIX {
            return None;
        }
        let mut lower = Zeroizing::new(typed.to_vec());
        lower.make_ascii_lowercase();
        let lower: &[u8] = &lower;
        let at = WORDS.partition_point(|w| w.as_bytes() < lower);
        // Sorted list: if any word starts with `lower`, the first word not
        // below it does; unique 3-letter prefixes make it the only one.
        if !WORDS.get(at)?.as_bytes().starts_with(lower) {
            return None;
        }
        u16::try_from(at).ok().map(Self)
    }

    fn text(self) -> &'static str {
        WORDS[usize::from(self.0)]
    }
}

/// Number of words, as the bound for [`uniform_below`] (checked at compile
/// time).
pub(super) const WORD_COUNT: NonZeroU16 = const_count(WORDLIST_LEN);
/// Number of nameplates, as the bound for [`uniform_below`].
pub(super) const NAMEPLATE_COUNT: NonZeroU16 =
    const_count((*NAMEPLATES.end() - *NAMEPLATES.start()) as usize + 1);

/// `n` as a non-zero `u16`; only evaluated in `const` items, so a bad
/// table size fails the build rather than panicking at run time.
const fn const_count(n: usize) -> NonZeroU16 {
    assert!(
        n > 0 && n <= u16::MAX as usize,
        "count must be 1..=u16::MAX"
    );
    match NonZeroU16::new(n as u16) {
        Some(n) => n,
        None => unreachable!(),
    }
}

/// Uniform integer in `0..n` from the OS RNG (rejection sampling over 16-bit
/// samples, so no modulo bias).
pub(super) fn uniform_below(n: NonZeroU16) -> Result<u16> {
    const SAMPLES: u32 = 1 << 16;
    // Largest multiple of `n` within the sample space.
    let zone = SAMPLES - SAMPLES % NonZeroU32::from(n);
    let mut buf = Zeroizing::new([0u8; 2]);
    loop {
        getrandom::fill(&mut *buf).map_err(|_| PairingError::Rng)?;
        let x = u16::from_be_bytes(*buf);
        if u32::from(x) < zone {
            return Ok(x % n);
        }
    }
}

/// One-time pairing code `<nameplate>-<word>-<word>` (e.g. `7-zebra-lemon`).
///
/// `Display` renders the canonical form for showing to the user; it is
/// never logged. Parsing is case-insensitive, trims, accepts spaces and/or
/// `-` as separators and unambiguous word prefixes of at least
/// [`MIN_WORD_PREFIX`] letters. `Debug` shows only the nameplate.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingCode {
    nameplate: Nameplate,
    words: Zeroizing<[Word; CODE_WORDS]>,
}

impl PairingCode {
    /// Fresh random code: nameplate uniform in [`NAMEPLATES`], each word
    /// uniform over the word list (OS RNG).
    pub fn generate() -> Result<Self> {
        let nameplate = Nameplate::new(NAMEPLATES.start() + uniform_below(NAMEPLATE_COUNT)?)?;
        let mut words = Zeroizing::new([Word(0); CODE_WORDS]);
        for word in words.iter_mut() {
            *word = Word::uniform()?;
        }
        Ok(Self { nameplate, words })
    }

    /// Public nameplate of this code.
    #[must_use]
    pub fn nameplate(&self) -> Nameplate {
        self.nameplate
    }

    /// Canonical SPAKE2 password bytes: [`PASSWORD_LABEL`] ‖ BE16 nameplate
    /// ‖ BE16 index of each word.
    pub(super) fn password_bytes(&self) -> Zeroizing<[u8; PASSWORD_LEN]> {
        let mut out = Zeroizing::new([0u8; PASSWORD_LEN]);
        let (label, rest) = out.split_at_mut(PASSWORD_LABEL.len());
        label.copy_from_slice(PASSWORD_LABEL);
        let values = std::iter::once(self.nameplate.get()).chain(self.words.iter().map(|w| w.0));
        for (slot, value) in rest.as_chunks_mut::<2>().0.iter_mut().zip(values) {
            *slot = value.to_be_bytes();
        }
        out
    }

    /// SPAKE2 password.
    pub(super) fn password(&self) -> Password {
        Password::new(*self.password_bytes())
    }
}

impl fmt::Display for PairingCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.nameplate)?;
        self.words
            .iter()
            .try_for_each(|w| write!(f, "{CODE_SEPARATOR}{}", w.text()))
    }
}

impl fmt::Debug for PairingCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairingCode")
            .field("nameplate", &self.nameplate)
            .field("words", &"<redacted>")
            .finish()
    }
}

impl FromStr for PairingCode {
    type Err = PairingError;

    fn from_str(s: &str) -> Result<Self> {
        let parts = || {
            s.split(|c: char| c == CODE_SEPARATOR || c.is_whitespace())
                .filter(|p| !p.is_empty())
        };
        if parts().count() != 1 + CODE_WORDS {
            return Err(CodeError::Shape.into());
        }
        let mut parts = parts();
        let nameplate = parts.next().ok_or(CodeError::Shape)?.parse::<Nameplate>()?;
        let mut words = Zeroizing::new([Word(0); CODE_WORDS]);
        for ((position, slot), typed) in (1..).zip(words.iter_mut()).zip(parts) {
            *slot = Word::parse(typed).ok_or(CodeError::UnknownWord { position })?;
        }
        Ok(Self { nameplate, words })
    }
}
