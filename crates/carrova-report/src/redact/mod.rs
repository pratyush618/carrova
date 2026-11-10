//! Redaction of free text (error messages, panic payloads) before it is
//! stored or shown in a report.
//!
//! Text is split into words at delimiters ([`lex`]); delimiters are kept
//! verbatim. Each word is checked, in order, first match wins:
//!
//! 1. the value of a secret key (`password=…`, `Bearer …`) →
//!    its [`keyed::KEYED_RULES`] placeholder,
//! 2. a URL → redacted by structure ([`url`]): credentials, host, path,
//!    query,
//! 3. a path in the user's home directory (or already written `~/…`) →
//!    `~/<path>` plus its file type ([`home`]),
//! 4. the [`rules::RULES`] table → its placeholder,
//! 5. failing that, each `:`-separated part against the table
//!    (`hash:abcd…`, `addr:10.0.0.1`),
//! 6. the user name as a whole word → `<user>`.
//!
//! Order matters: keyed values are replaced whatever they look like, URLs
//! before paths so `file://` and `https://` are taken apart first, and the
//! home rule before the table's generic "absolute path". Trailing sentence
//! punctuation (`.,:;!?`) is set aside before matching and re-appended.
//! Every placeholder survives re-redaction (redaction is idempotent).

mod home;
mod keyed;
mod lex;
mod rules;
mod url;

use std::path::Path;

use lex::Piece;

/// Sentence punctuation stripped from a word's end before matching.
const TRAILING_PUNCTUATION: &[char] = &['.', ',', ':', ';', '!', '?'];
/// Placeholder for the user name.
const USER_PLACEHOLDER: &str = "<user>";
/// User names shorter than this are not redacted as words (too many false hits).
const MIN_USER_LEN: usize = 2;

/// Applies Carrova's redaction policy to free text.
///
/// Built from the user's home directory, which the caller resolves through
/// `carrova-platform`; the user name is taken from its last component.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    home: Option<String>,
    user: Option<String>,
}

impl Redactor {
    /// A redactor for a user whose home directory is `home` (`None` when
    /// unknown: only the context-free rules then apply).
    #[must_use]
    pub fn new(home: Option<&Path>) -> Self {
        let home = home
            .map(|path| {
                path.to_string_lossy()
                    .trim_end_matches(['/', '\\'])
                    .to_owned()
            })
            .filter(|path| !path.is_empty());
        let user = home
            .as_deref()
            .and_then(|path| path.rsplit(['/', '\\']).next())
            .filter(|name| name.len() >= MIN_USER_LEN)
            .map(str::to_owned);
        Self { home, user }
    }

    /// The policy as `(what, placeholder)` pairs, in match order, e.g.
    /// for a consent dialog explaining what reports omit.
    pub fn policy() -> impl Iterator<Item = (&'static str, &'static str)> {
        let keyed = keyed::KEYED_RULES
            .iter()
            .map(|rule| (rule.name, rule.replacement));
        let tokens = rules::RULES
            .iter()
            .map(|rule| (rule.name, rule.replacement));
        keyed
            .chain([("URL credentials", url::CREDENTIALS)])
            .chain([("path under the home directory", "~/<path>")])
            .chain(tokens)
            .chain([("user name", USER_PLACEHOLDER)])
    }

    /// Returns `text` with every sensitive word replaced by a placeholder.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut tracker = keyed::Tracker::default();
        for piece in lex::pieces(text) {
            match piece {
                Piece::Delimiter(ch) => {
                    tracker.delimiter(ch);
                    out.push(ch);
                }
                Piece::Word(word) => {
                    let (core, suffix) = split_trailing(word);
                    match tracker.word(core, suffix) {
                        Some(rule) => {
                            out.push_str(rule.replacement);
                            out.push_str(suffix);
                        }
                        None => out.push_str(&self.redact_word(word)),
                    }
                }
            }
        }
        out
    }

    /// Steps 2–6 for one word.
    fn redact_word(&self, word: &str) -> String {
        let (core, suffix) = split_trailing(word);
        // Prefer the trimmed core; fall back to the whole word for forms
        // that end in punctuation themselves (`fe80::`).
        if let Some(redacted) = self.redact_whole(core) {
            return format!("{redacted}{suffix}");
        }
        if let Some(redacted) = self.redact_whole(word) {
            return redacted;
        }
        format!("{}{suffix}", self.redact_parts(core))
    }

    /// Steps 2–4. `None` = no match.
    fn redact_whole(&self, token: &str) -> Option<String> {
        if token.is_empty() {
            return None;
        }
        if let Some(redacted) = self.redact_url(token) {
            return Some(redacted);
        }
        if let Some(tail) = self.home_tail(token) {
            return Some(home::collapse(tail));
        }
        rules::classify(token).map(|rule| rule.replacement.to_owned())
    }

    /// Steps 5–6 for a token no rule matched as a whole.
    fn redact_parts(&self, token: &str) -> String {
        token
            .split(':')
            .map(|part| match rules::classify(part) {
                Some(rule) => rule.replacement.to_owned(),
                None => self.redact_user(part),
            })
            .collect::<Vec<_>>()
            .join(":")
    }

    /// The part of `token` after the home directory (starting with a
    /// separator, or empty for the home directory itself), whether written
    /// out or already as `~`.
    fn home_tail<'t>(&self, token: &'t str) -> Option<&'t str> {
        let tail = self
            .home
            .as_deref()
            .and_then(|home| token.strip_prefix(home))
            .or_else(|| token.strip_prefix(home::HOME))?;
        (tail.is_empty() || tail.starts_with(['/', '\\'])).then_some(tail)
    }

    /// Replaces the user name where it stands as a whole word.
    fn redact_user(&self, text: &str) -> String {
        let Some(user) = self.user.as_deref() else {
            return text.to_owned();
        };
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find(user) {
            let before = rest[..at].chars().next_back();
            let after = rest[at + user.len()..].chars().next();
            out.push_str(&rest[..at]);
            if is_word_char(before) || is_word_char(after) {
                out.push_str(user);
            } else {
                out.push_str(USER_PLACEHOLDER);
            }
            rest = &rest[at + user.len()..];
        }
        out.push_str(rest);
        out
    }
}

/// Splits trailing sentence punctuation off `word`.
fn split_trailing(word: &str) -> (&str, &str) {
    let core = word.trim_end_matches(TRAILING_PUNCTUATION);
    word.split_at(core.len())
}

fn is_word_char(ch: Option<char>) -> bool {
    ch.is_some_and(|c| c.is_alphanumeric() || c == '_')
}
