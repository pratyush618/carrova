//! `gpg-keyring`: public keys + ownertrust through `gpg` itself, so the
//! receiver's keyring is merged, not overwritten. Secret keys are not
//! handled here (they move as raw files).

use std::path::{Path, PathBuf};
use std::process::Command;

use carrova_platform::{FileKind, Platform};
use carrova_recipe::Handler;

use super::{ArtifactData, run};
use crate::error::{CoreError, Result};

/// This handler.
const HANDLER: Handler = Handler::GpgKeyring;
const PUBRING: &str = "pubring.asc";
const OWNERTRUST: &str = "ownertrust.txt";

fn err(reason: impl Into<String>) -> CoreError {
    CoreError::Handler {
        handler: HANDLER.as_str().into(),
        reason: reason.into(),
    }
}

/// The GnuPG home the handler reads and writes ([`Platform::gnupg_home`]).
pub(super) fn gnupg_home(p: &dyn Platform) -> Result<PathBuf> {
    Ok(p.gnupg_home()?)
}

/// `gpg` with the platform's minimal tool environment (no shell, no
/// inherited variables) plus an explicit `GNUPGHOME`.
fn gpg(p: &dyn Platform) -> Result<Command> {
    let bin = p
        .find_binary("gpg")?
        .ok_or_else(|| err("gpg is not installed"))?;
    let mut cmd = p.tool_command(&bin)?;
    cmd.env("GNUPGHOME", gnupg_home(p)?)
        .args(["--batch", "--no-tty"]);
    Ok(cmd)
}

pub(super) fn export(p: &dyn Platform) -> Result<Vec<ArtifactData>> {
    let mut c = gpg(p)?;
    c.args(["--export", "--armor"]);
    let pubring = run(HANDLER, c)?;
    let mut c = gpg(p)?;
    c.arg("--export-ownertrust");
    let trust = run(HANDLER, c)?;
    Ok(vec![
        ArtifactData {
            name: PUBRING.into(),
            bytes: pubring,
        },
        ArtifactData {
            name: OWNERTRUST.into(),
            bytes: trust,
        },
    ])
}

pub(super) fn import(
    p: &dyn Platform,
    artifacts: &[(String, &Path)],
    import_trust: bool,
) -> Result<()> {
    let mut c = gpg(p)?; // fail early (and before touching anything) if gpg is missing
    let home = gnupg_home(p)?;
    match crate::fsx::lstat(p, &home)? {
        Some(m) if m.kind == FileKind::Dir => {}
        Some(_) => return Err(err("GnuPG home is not a directory")),
        None => p.create_dir(&home, Some(0o700))?,
    }
    let find = |n: &str| {
        artifacts
            .iter()
            .find(|(name, _)| name == n)
            .map(|(_, p)| *p)
    };
    for (name, _) in artifacts {
        if name != PUBRING && name != OWNERTRUST {
            return Err(err(format!("unexpected artifact {name}")));
        }
    }
    let non_empty = |path: &Path| p.metadata(path).map(|m| m.size > 0);
    if let Some(path) = find(PUBRING)
        && non_empty(path)?
    {
        c.arg("--import").arg(path);
        run(HANDLER, c)?;
    }
    // Trust levels from an archive could mark an attacker's key as trusted:
    // imported only when the user explicitly opted in.
    if import_trust
        && let Some(path) = find(OWNERTRUST)
        && non_empty(path)?
    {
        let mut c = gpg(p)?;
        c.arg("--import-ownertrust").arg(path);
        run(HANDLER, c)?;
    }
    Ok(())
}
