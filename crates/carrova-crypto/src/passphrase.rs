//! User passphrase secret.

/// User passphrase. Zeroized on drop; `Debug` prints `Passphrase(***)`.
pub struct Passphrase(secrecy::SecretString);

impl Passphrase {
    /// Wrap a passphrase.
    #[must_use]
    pub fn new(s: String) -> Self {
        Self(secrecy::SecretString::from(s))
    }

    /// Expose the secret (only for KDF input).
    #[must_use]
    pub fn expose(&self) -> &str {
        secrecy::ExposeSecret::expose_secret(&self.0)
    }
}

impl std::fmt::Debug for Passphrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Passphrase(***)")
    }
}
