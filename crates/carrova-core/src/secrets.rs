//! Well-known secret locations: the one table that marks data without a
//! recipe as sensitive, on the sender (Manual picks) and on the receiver
//! (raw items, whatever the source claims).

/// How a path is matched against a [`SECRET_NAMES`] row, at one of its
/// components. Names are compared ignoring ASCII case (over-flagging is
/// safe); the table spells them in lower case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretRule {
    /// The component is exactly this name.
    Name(&'static str),
    /// The component ends with this suffix.
    Suffix(&'static str),
    /// The component starts with this prefix and is not a public key
    /// (`.pub`).
    PrivateKeyPrefix(&'static str),
    /// These `/`-separated components in a row, wherever they sit, so one
    /// row matches under any root: `gh/hosts.yml` is `user_config:gh/…`
    /// and `home:.config/gh/…` alike.
    Components(&'static str),
}

impl SecretRule {
    /// Whether the path components starting at `names[0]` match this rule.
    #[must_use]
    pub fn matches(self, names: &[&str]) -> bool {
        let Some(name) = names.first().map(|n| n.to_ascii_lowercase()) else {
            return false;
        };
        match self {
            Self::Name(n) => name == n,
            Self::Suffix(s) => name.ends_with(s),
            Self::PrivateKeyPrefix(p) => name.starts_with(p) && !name.ends_with(".pub"),
            Self::Components(c) => {
                let want: Vec<&str> = c.split('/').collect();
                want.len() <= names.len()
                    && want
                        .iter()
                        .zip(names)
                        .all(|(w, n)| n.eq_ignore_ascii_case(w))
            }
        }
    }
}

/// Secret locations, with why each is secret in the user's words.
pub const SECRET_NAMES: &[(SecretRule, &str)] = &[
    (SecretRule::Name(".ssh"), "SSH keys"),
    (SecretRule::Name(".gnupg"), "GnuPG keys"),
    (SecretRule::Name(".aws"), "cloud credentials"),
    (SecretRule::Name(".azure"), "cloud credentials"),
    (SecretRule::Name("gcloud"), "cloud credentials"),
    (SecretRule::Name(".kube"), "cluster credentials"),
    (SecretRule::PrivateKeyPrefix("id_"), "a private key"),
    (SecretRule::Suffix(".pem"), "a key or certificate"),
    (SecretRule::Suffix(".key"), "a private key"),
    (SecretRule::Name(".env"), "environment secrets"),
    (SecretRule::Name(".netrc"), "saved passwords"),
    (SecretRule::Name(".pgpass"), "saved database passwords"),
    (SecretRule::Name(".git-credentials"), "saved git passwords"),
    (
        SecretRule::Components("gh/hosts.yml"),
        "GitHub sign-in tokens",
    ),
    (
        SecretRule::Components(".docker/config.json"),
        "container registry passwords",
    ),
    (SecretRule::Name(".npmrc"), "package registry tokens"),
    (SecretRule::Name(".pypirc"), "package registry tokens"),
    (
        SecretRule::Name("credentials.tfrc.json"),
        "Terraform tokens",
    ),
    (SecretRule::Name("rclone.conf"), "cloud storage passwords"),
    (SecretRule::Name(".password-store"), "a password store"),
    (SecretRule::Name("keyrings"), "saved passwords (keyring)"),
    (SecretRule::Name("key4.db"), "browser saved passwords"),
    (SecretRule::Name("logins.json"), "browser saved passwords"),
    (SecretRule::Name("login data"), "browser saved passwords"),
];

/// Why `rel` (a `/`-separated path under a semantic root) is or lies in a
/// secret location: the reason of the first matching component, `None`
/// when no component matches.
#[must_use]
pub fn secret_in(rel: &str) -> Option<&'static str> {
    let names: Vec<&str> = rel.split('/').collect();
    (0..names.len()).find_map(|i| {
        SECRET_NAMES
            .iter()
            .find(|(rule, _)| rule.matches(&names[i..]))
            .map(|&(_, why)| why)
    })
}

#[cfg(test)]
#[path = "../tests/secrets/unit.rs"]
mod tests;
