//! Secret key files: a small `key=value` text format, so the secret never
//! passes through a serializer that could leave copies behind.
//!
//! ```text
//! # Carrova bundle root key. SECRET: keep offline.
//! role=root
//! public=<64 hex>
//! secret=<64 hex>
//! ```

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, bail, ensure};
use carrova_crypto::sign::SigningKey;
use zeroize::Zeroizing;

/// What a key is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Role {
    /// Offline root: certifies signer keys.
    Root,
    /// Bundle signer (CI): signs bundle indexes.
    Signer,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Signer => "signer",
        }
    }

    fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "root" => Ok(Self::Root),
            "signer" => Ok(Self::Signer),
            other => bail!("unknown key role `{other}`"),
        }
    }
}

/// Create `path` (mode 0600, must not exist) holding `key`.
pub(crate) fn write(path: &Path, role: Role, key: &SigningKey) -> anyhow::Result<()> {
    let text = Zeroizing::new(format!(
        "# Carrova bundle {role} key. SECRET: never commit, never share.\n\
         role={role}\npublic={public}\nsecret={secret}\n",
        role = role.as_str(),
        public = key.verifying_key(),
        secret = key.to_secret_hex().as_str(),
    ));
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let mut file = carrova_platform::current()
        .create_new_file(path, Some(0o600))
        .with_context(|| format!("create {}", path.display()))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .with_context(|| format!("write {}", path.display()))
}

/// Read a key file, requiring `role` and a public key matching the secret.
pub(crate) fn read(path: &Path, role: Role) -> anyhow::Result<SigningKey> {
    let text = Zeroizing::new(
        fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?,
    );
    let (mut found_role, mut public, mut key) = (None, None, None);
    for line in text.lines().filter(|l| !l.trim_start().starts_with('#')) {
        match line.split_once('=') {
            Some(("role", v)) => found_role = Some(Role::parse(v.trim())?),
            Some(("public", v)) => public = Some(v.trim().to_owned()),
            Some(("secret", v)) => {
                key = Some(SigningKey::from_secret_hex(v).context("malformed secret key")?);
            }
            _ => {}
        }
    }
    let (Some(found_role), Some(key)) = (found_role, key) else {
        bail!("{} is not a Carrova key file", path.display());
    };
    ensure!(
        found_role == role,
        "{} holds a {} key, expected a {} key",
        path.display(),
        found_role.as_str(),
        role.as_str()
    );
    ensure!(
        public.as_deref() == Some(key.verifying_key().to_string().as_str()),
        "{}: public key does not match the secret",
        path.display()
    );
    Ok(key)
}

#[cfg(test)]
#[path = "../../tests/bundle/keyfile/unit.rs"]
mod tests;
