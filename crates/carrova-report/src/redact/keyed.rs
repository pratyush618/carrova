//! Values that are secret because of the key in front of them
//! (`password=…`, `token: …`, `Bearer …`), whatever they look like.
//!
//! [`Tracker`] follows the word/delimiter stream of [`super::Redactor`]
//! and says when a word is the value of a [`KEYED_RULES`] key.

/// What separates a key from its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Separator {
    /// `key=value`, `key: value`, `"key": "value"` (spaces/quotes allowed).
    Assign,
    /// `Key value`: whitespace only.
    Space,
}

/// One row of the keyed policy.
#[derive(Debug)]
pub(crate) struct KeyedRule {
    /// What the rule protects (shown by [`super::Redactor::policy`]).
    pub(crate) name: &'static str,
    /// Lower-case keys. A word matches as itself or as a `_`/`-`/`.`
    /// suffix (`GITHUB_TOKEN`, `db.password`), leading dashes ignored
    /// (`--password`).
    keys: &'static [&'static str],
    separator: Separator,
    /// Which values are redacted (others go through the token rules).
    value: fn(&str) -> bool,
    /// Placeholder written instead of the value.
    pub(crate) replacement: &'static str,
}

/// The keyed policy, first key match wins.
pub(crate) const KEYED_RULES: &[KeyedRule] = &[
    KeyedRule {
        name: "bearer credential",
        keys: &["bearer"],
        separator: Separator::Space,
        value: any_value,
        replacement: "<secret>",
    },
    KeyedRule {
        name: "AWS secret access key",
        keys: &["aws_secret_access_key"],
        separator: Separator::Assign,
        value: is_aws_secret,
        replacement: "<secret>",
    },
    KeyedRule {
        name: "assigned credential",
        keys: &[
            "password",
            "passwd",
            "pwd",
            "token",
            "secret",
            "api_key",
            "apikey",
            "access_token",
            "refresh_token",
            "client_secret",
            "private_key",
        ],
        separator: Separator::Assign,
        value: any_value,
        replacement: "<secret>",
    },
];

/// Quotes a key and its value may be wrapped in.
const QUOTES: &[char] = &['"', '\'', '`'];
/// Length of an AWS secret access key.
const AWS_SECRET_LEN: usize = 40;

fn any_value(_: &str) -> bool {
    true
}

/// 40 characters of standard base64.
fn is_aws_secret(value: &str) -> bool {
    value.len() == AWS_SECRET_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/'))
}

impl KeyedRule {
    fn matches_key(&self, word: &str) -> bool {
        let word = word.trim_start_matches('-');
        self.keys.iter().any(|key| {
            word.len() >= key.len()
                && word.is_char_boundary(word.len() - key.len())
                && word[word.len() - key.len()..].eq_ignore_ascii_case(key)
                && word[..word.len() - key.len()]
                    .chars()
                    .next_back()
                    .is_none_or(|ch| matches!(ch, '_' | '-' | '.'))
        })
    }
}

/// The `Assign` rule whose key `word` is and that accepts `value`, for
/// `key=value` pairs seen as one unit (URL query strings).
pub(crate) fn assigned_secret(word: &str, value: &str) -> Option<&'static KeyedRule> {
    KEYED_RULES.iter().find(|rule| {
        rule.separator == Separator::Assign && rule.matches_key(word) && (rule.value)(value)
    })
}

/// Where the stream stands relative to a secret key.
#[derive(Debug, Clone, Copy, Default)]
enum State {
    #[default]
    Idle,
    /// A key was seen; its separator has not been yet.
    Key(&'static KeyedRule),
    /// The next word is the key's value.
    Value(&'static KeyedRule),
}

/// Follows words and delimiters to spot values of secret keys.
#[derive(Debug, Default)]
pub(crate) struct Tracker {
    state: State,
}

impl Tracker {
    /// Feeds a word, split into `core` and trailing punctuation `suffix`.
    /// Returns the rule whose value `core` is, when it must be redacted.
    pub(crate) fn word(&mut self, core: &str, suffix: &str) -> Option<&'static KeyedRule> {
        match self.state {
            State::Value(rule) => {
                self.state = State::Idle;
                return (rule.value)(core).then_some(rule);
            }
            // A lone `:` between key and value (`"password": "…"`).
            State::Key(rule)
                if rule.separator == Separator::Assign && core.is_empty() && suffix == ":" =>
            {
                self.state = State::Value(rule);
                return None;
            }
            State::Key(_) | State::Idle => {}
        }
        self.state = KEYED_RULES
            .iter()
            .find(|rule| rule.matches_key(core))
            .map_or(State::Idle, |rule| match (rule.separator, suffix) {
                (Separator::Assign, ":") => State::Value(rule),
                (_, "") => State::Key(rule),
                _ => State::Idle,
            });
        None
    }

    /// Feeds a delimiter character.
    pub(crate) fn delimiter(&mut self, ch: char) {
        let quote_or_space = ch.is_whitespace() || QUOTES.contains(&ch);
        self.state = match self.state {
            State::Key(rule) if rule.separator == Separator::Assign && ch == '=' => {
                State::Value(rule)
            }
            State::Key(rule) if rule.separator == Separator::Space && ch.is_whitespace() => {
                State::Value(rule)
            }
            state @ (State::Key(_) | State::Value(_)) if quote_or_space => state,
            _ => State::Idle,
        };
    }
}
