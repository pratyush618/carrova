//! The token rules: one table, checked in order.
//!
//! Every rule classifies a single token (text between delimiters, see
//! [`super::Redactor`]) and names the placeholder that replaces it. Adding
//! a rule = adding a row; a matcher is either a known prefix plus a body
//! shape, or a small pure predicate.
//!
//! * [`table`]: the rows ([`RULES`]), in match order.
//! * [`matchers`]: the shape predicates the rows name.

mod matchers;
mod table;

pub(crate) use table::RULES;

/// One row of the token policy.
#[derive(Debug)]
pub(crate) struct Rule {
    /// What the rule protects (shown by [`super::Redactor::policy`]).
    pub(crate) name: &'static str,
    /// Which tokens are sensitive.
    pub(crate) matcher: Matcher,
    /// Placeholder written instead of the token.
    pub(crate) replacement: &'static str,
}

/// How a [`Rule`] recognises a sensitive token.
#[derive(Debug)]
pub(crate) enum Matcher {
    /// One of `prefixes`, then a body of `body_len` bytes of `charset`.
    Prefixed {
        prefixes: &'static [&'static str],
        body_len: (usize, usize),
        charset: Charset,
    },
    /// A predicate over the whole token.
    Shape(fn(&str) -> bool),
}

/// Byte classes a token body may consist of.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Charset {
    /// `[A-Za-z0-9_]`
    Word,
    /// `[A-Za-z0-9_-]`
    WordDash,
    /// `[A-Za-z0-9-]`
    AlnumDash,
    /// `[A-Z0-9]`
    UpperAlnum,
}

impl Charset {
    pub(super) fn admits(self, byte: u8) -> bool {
        match self {
            Self::Word => byte.is_ascii_alphanumeric() || byte == b'_',
            Self::WordDash => byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'),
            Self::AlnumDash => byte.is_ascii_alphanumeric() || byte == b'-',
            Self::UpperAlnum => byte.is_ascii_uppercase() || byte.is_ascii_digit(),
        }
    }
}

impl Matcher {
    fn matches(&self, token: &str) -> bool {
        match self {
            Self::Prefixed {
                prefixes,
                body_len: (min, max),
                charset,
            } => prefixes.iter().any(|prefix| {
                token.strip_prefix(prefix).is_some_and(|body| {
                    (*min..=*max).contains(&body.len()) && body.bytes().all(|b| charset.admits(b))
                })
            }),
            Self::Shape(predicate) => predicate(token),
        }
    }
}

/// Finds the first rule matching `token`.
pub(crate) fn classify(token: &str) -> Option<&'static Rule> {
    RULES.iter().find(|rule| rule.matcher.matches(token))
}

#[cfg(test)]
#[path = "../../../tests/redact/rules/unit.rs"]
mod tests;
