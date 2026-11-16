//! The WirePlumber drop-in that hands Bluetooth HFP to oFono, in the
//! format of the installed WirePlumber (0.4: Lua, 0.5+: SPA-JSON).

use std::path::{Path, PathBuf};
use std::process::Command;

use super::super::LinuxPlatform;
use super::super::file_manager::remove_if_exists;
use crate::PlatformError;

/// Drop-in paths, relative to the user's config dir.
const LUA: &str = "wireplumber/bluetooth.lua.d/51-carrova-hfp.lua";
const CONF: &str = "wireplumber/wireplumber.conf.d/51-carrova-hfp.conf";

// shortcut: unverified on hardware until spike S4; update from its result
const LUA_TEXT: &str = r#"-- Written by Carrova: phone calls through this computer (oFono hands-free).
bluez_monitor.properties["bluez5.hfphsp-backend"] = "ofono"
"#;

// shortcut: unverified on hardware until spike S4; update from its result
const CONF_TEXT: &str = r#"# Written by Carrova: phone calls through this computer (oFono hands-free).
monitor.bluez.properties = {
  bluez5.hfphsp-backend = "ofono"
}
"#;

/// The drop-in for WirePlumber `version`: Lua before 0.5, SPA-JSON from
/// 0.5 on (and for a version it cannot read: newer is likelier).
pub(super) fn drop_in(version: &str) -> (PathBuf, &'static str) {
    let mut parts = version.split('.').map(str::parse::<u32>);
    let lua = matches!(
        (parts.next(), parts.next()),
        (Some(Ok(0)), Some(Ok(minor))) if minor < 5
    );
    if lua {
        (PathBuf::from(LUA), LUA_TEXT)
    } else {
        (PathBuf::from(CONF), CONF_TEXT)
    }
}

/// The library version in `wireplumber --version` output.
pub(super) fn parse_version(output: &str) -> Option<&str> {
    output
        .lines()
        .find_map(|l| l.strip_prefix("Linked with libwireplumber "))
        .map(str::trim)
}

/// The installed WirePlumber's version; `None` if it does not answer.
pub(super) fn installed_version() -> Option<String> {
    let out = crate::tool::stdout_of(Command::new("wireplumber").arg("--version"))?;
    parse_version(&out).map(str::to_owned)
}

/// Write the drop-in for `version`, atomically; rewriting is harmless.
pub(super) fn write(p: &LinuxPlatform, version: &str) -> Result<(), PlatformError> {
    let (rel, text) = drop_in(version);
    p.write_atomic(&p.config_dir()?.join(rel), text, 0o644)
}

/// Whether the drop-in for `version` is in place.
pub(super) fn configured(p: &LinuxPlatform, version: &str) -> Result<bool, PlatformError> {
    Ok(p.config_dir()?.join(drop_in(version).0).exists())
}

/// Remove either drop-in; a missing one is no error.
pub(super) fn remove(p: &LinuxPlatform) -> Result<(), PlatformError> {
    let dir = p.config_dir()?;
    for rel in [LUA, CONF] {
        remove_if_exists(p, &dir.join(Path::new(rel)))?;
    }
    Ok(())
}

/// Restart the user's WirePlumber so it reads the drop-in; `--no-block`
/// so a slow stop cannot outlast the tool deadline.
pub(super) fn restart() -> Result<(), PlatformError> {
    crate::tool::stdout_of(Command::new("systemctl").args([
        "--user",
        "--no-block",
        "restart",
        "wireplumber",
    ]))
    .map(drop)
    .ok_or_else(|| {
        PlatformError::io(
            "systemctl --user restart wireplumber",
            std::io::Error::other("restart failed"),
        )
    })
}
