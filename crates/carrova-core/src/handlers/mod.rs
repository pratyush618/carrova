//! Built-in handlers named by recipe schema v2 (`handler: <name>`).
//! Recipes stay data: they only *name* a handler; the logic is here.
//! Handlers run external tools via [`Platform::tool_command`] (minimal,
//! OS-appropriate environment) plus tool-specific variables (e.g.
//! `GNUPGHOME` from [`Platform::gnupg_home`]), never a shell, and never log
//! tool output that could contain secrets.

mod gpg;

use std::path::{Path, PathBuf};

use carrova_platform::Platform;
use carrova_recipe::Handler;

use crate::error::{CoreError, Result};

/// One named handler output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactData {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Run handler `name`'s export on this machine (`name` as a recipe or
/// manifest spells it; an unknown name is a [`CoreError::Handler`]).
pub fn export(platform: &dyn Platform, name: &str) -> Result<Vec<ArtifactData>> {
    match Handler::from_name(name) {
        Some(Handler::GpgKeyring) => gpg::export(platform),
        None => Err(CoreError::Handler {
            handler: name.to_owned(),
            reason: "unknown handler".into(),
        }),
    }
}

/// Run `handler`'s import with staged artifacts (`(name, path)`).
/// Caller has already backed up the handler's declared paths.
///
/// `import_trust`: also import trust settings (gpg ownertrust). The engine
/// passes it only when the user opted in.
pub fn import(
    platform: &dyn Platform,
    handler: Handler,
    artifacts: &[(String, &Path)],
    import_trust: bool,
) -> Result<()> {
    match handler {
        Handler::GpgKeyring => gpg::import(platform, artifacts, import_trust),
    }
}

/// Directories `handler` writes into on import (e.g. `GNUPGHOME`).
/// Apply creates missing ones (mode `700`, journalled) before [`import`].
pub fn required_dirs(platform: &dyn Platform, handler: Handler) -> Result<Vec<PathBuf>> {
    match handler {
        Handler::GpgKeyring => Ok(vec![gpg::gnupg_home(platform)?]),
    }
}

/// Whether `name` is a known built-in handler.
#[must_use]
pub fn is_known(name: &str) -> bool {
    Handler::from_name(name).is_some()
}

/// Run `cmd` (no shell); on failure return a [`CoreError::Handler`] with a
/// truncated stderr excerpt. stdout is returned, never logged.
fn run(handler: Handler, mut cmd: std::process::Command) -> Result<Vec<u8>> {
    use std::process::Stdio;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = cmd.output().map_err(|e| CoreError::Handler {
        handler: handler.as_str().into(),
        reason: format!("cannot run tool: {e}"),
    })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let mut excerpt: String = err.chars().take(300).collect();
        if err.chars().count() > 300 {
            excerpt.push('…');
        }
        return Err(CoreError::Handler {
            handler: handler.as_str().into(),
            reason: format!("tool exited with {}: {}", out.status, excerpt.trim()),
        });
    }
    Ok(out.stdout)
}
