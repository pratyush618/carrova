//! `build`, `sign` and `verify`.

use std::fs;
use std::path::Path;

use anyhow::{Context, ensure};
use carrova_bundle::{
    Asset, BlobHash, BundleIndex, BundleVersion, DirFetcher, Tier, VerifyTime, open_bundle,
    parse_roots,
};
use carrova_crypto::sign::SignedKeyCert;

use super::keyfile::{self, Role};

/// Build an unsigned bundle.
pub(super) fn build(recipes: &Path, out: &Path, version: u64, now: u64) -> anyhow::Result<()> {
    let report = carrova_bundle::build_bundle(recipes, out, BundleVersion(version), now)?;
    eprintln!(
        "built bundle {} with {} recipes ({} files) in {}",
        report.index.version,
        report.index.recipes.len(),
        report.files,
        out.display()
    );
    Ok(())
}

/// Sign a built bundle; the certificate must be valid now.
pub(super) fn sign(dir: &Path, signer: &Path, cert: &Path, now: u64) -> anyhow::Result<()> {
    let signer = keyfile::read(signer, Role::Signer)?;
    let text = fs::read_to_string(cert).with_context(|| format!("read {}", cert.display()))?;
    let cert: SignedKeyCert = serde_json::from_str(&text).context("malformed certificate")?;
    ensure!(
        (cert.cert.not_before..=cert.cert.not_after).contains(&now),
        "signer certificate is not valid now (valid {}..={}, now {now})",
        cert.cert.not_before,
        cert.cert.not_after
    );
    carrova_bundle::sign_bundle(dir, &signer, cert)?;
    eprintln!("signed {} with signer {}", dir.display(), cert.cert.key);
    Ok(())
}

/// Verify a signed bundle against `roots` at `now`, every blob included.
pub(super) fn verify(dir: &Path, roots: &Path, now: u64) -> anyhow::Result<()> {
    let text = fs::read_to_string(roots).with_context(|| format!("read {}", roots.display()))?;
    let roots = parse_roots(&text)?;
    let (signed, catalog) = open_bundle(&DirFetcher::new(dir), &roots, VerifyTime::Now(now))?;
    let verified = catalog
        .recipes
        .iter()
        .filter(|r| r.tier == Tier::Verified)
        .count();
    println!(
        "ok: bundle {} — {} recipes ({verified} verified), {} skipped (unsupported schema or failing validation)",
        signed.version(),
        catalog.recipes.len(),
        catalog.skipped.len()
    );
    Ok(())
}

/// Print the BLAKE3 of the bundle's `index.json` (the shipped-bundle pin).
pub(super) fn pin(dir: &Path) -> anyhow::Result<()> {
    let path = dir.join(Asset::INDEX_FILE);
    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    BundleIndex::parse(&bytes)?;
    println!("{}", BlobHash::of(&bytes));
    Ok(())
}
