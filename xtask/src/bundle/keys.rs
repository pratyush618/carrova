//! `keygen` and `certify`.

use std::fs;
use std::path::Path;

use anyhow::{Context, ensure};
use carrova_crypto::sign::{
    CERT_CLOCK_SKEW, KeyCert, KeyRole, MAX_CERT_VALIDITY_DAYS, SigningKey, VerifyingKey,
};

use super::keyfile::{self, Role};

const DAY_SECS: u64 = 86_400;

/// Generate a key, write its secret file, print the public key.
pub(super) fn keygen(role: Role, out: &Path) -> anyhow::Result<()> {
    let key = SigningKey::generate()?;
    keyfile::write(out, role, &key)?;
    println!("{}", key.verifying_key());
    eprintln!("secret key written to {} (mode 0600)", out.display());
    Ok(())
}

/// Certify `signer_pub` as a bundle signer for `days` days from `now`.
pub(super) fn certify(
    root: &Path,
    signer_pub: &str,
    days: u64,
    out: &Path,
    now: u64,
) -> anyhow::Result<()> {
    ensure!(
        (1..=MAX_CERT_VALIDITY_DAYS).contains(&days),
        "--days must be 1..={MAX_CERT_VALIDITY_DAYS}"
    );
    let root = keyfile::read(root, Role::Root)?;
    let key: VerifyingKey = signer_pub.parse().context("--signer-pub")?;
    let cert = KeyCert {
        key,
        role: KeyRole::BundleSigner,
        not_before: now.saturating_sub(CERT_CLOCK_SKEW),
        not_after: now + days * DAY_SECS,
    }
    .sign(&root)?;
    let json = serde_json::to_string_pretty(&cert)?;
    fs::write(out, json + "\n").with_context(|| format!("write {}", out.display()))?;
    eprintln!(
        "certified {key} as bundle signer until unix {} ({days} days) by root {}",
        cert.cert.not_after, cert.root
    );
    Ok(())
}
