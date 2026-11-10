//! [`PluginManifest`]: `plugin.yaml`, the data a user consents to.

use std::collections::HashSet;

use carrova_recipe::PluginId;
use serde::{Deserialize, Serialize};

use crate::PluginError;

/// The `plugin.yaml` schema this build reads.
pub const MANIFEST_SCHEMA: u32 = 1;
/// Hard cap on [`Limits::fuel`] (wasmi fuel units, roughly instructions;
/// about one second on a fast laptop).
pub const MAX_FUEL: u64 = 1_000_000_000;
/// Hard cap on [`Limits::memory_mib`].
pub const MAX_MEMORY_MIB: u32 = 256;
/// Longest accepted `version` / `description`, in bytes.
const MAX_TEXT: usize = 300;

/// Contents of `plugin.yaml`. Parsed with [`Self::parse`], which also
/// validates it; shown to the user as [`Self::permissions`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    /// Manifest schema ([`MANIFEST_SCHEMA`]).
    pub schema: u32,
    /// Plugin id; recipes name it as `handler: { plugin: <id> }`.
    pub id: PluginId,
    /// Plugin release, e.g. `1.0.0` (one line).
    pub version: String,
    /// What the plugin does, in one sentence (shown to the user).
    pub description: String,
    /// The only facts the plugin is told, in this order.
    #[serde(default)]
    pub context: Vec<ContextField>,
    /// Per-file resource limits.
    pub limits: Limits,
}

/// A fact a plugin may be told about the transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextField {
    /// Home folder path on the computer the data came from.
    SourceHome,
    /// Home folder path on this computer.
    TargetHome,
    /// OS family the data came from.
    SourceOs,
    /// OS family of this computer.
    TargetOs,
}

impl ContextField {
    /// Key in the encoded context (same as the YAML spelling).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceHome => "source_home",
            Self::TargetHome => "target_home",
            Self::SourceOs => "source_os",
            Self::TargetOs => "target_os",
        }
    }

    /// What the user is told the plugin sees.
    const fn permission(self) -> &'static str {
        match self {
            Self::SourceHome => "It sees the path of your home folder on the old computer.",
            Self::TargetHome => "It sees the path of your home folder on this computer.",
            Self::SourceOs => "It sees which operating system the old computer runs.",
            Self::TargetOs => "It sees which operating system this computer runs.",
        }
    }
}

/// Limits for one file's transform; checked against the hard caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Fuel per file, 1..=[`MAX_FUEL`].
    pub fuel: u64,
    /// Linear memory, 1..=[`MAX_MEMORY_MIB`] MiB.
    pub memory_mib: u32,
}

impl PluginManifest {
    /// Parse and [`Self::validate`] `plugin.yaml` text.
    pub fn parse(text: &str) -> Result<Self, PluginError> {
        let manifest: Self =
            serde_yaml_ng::from_str(text).map_err(|e| PluginError::Manifest(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// The rules a schema cannot express: schema, one-line text, unique
    /// context fields, limits within the hard caps.
    pub fn validate(&self) -> Result<(), PluginError> {
        let bad = |why: String| Err(PluginError::Manifest(why));
        if self.schema != MANIFEST_SCHEMA {
            return bad(format!(
                "schema {} is not supported (expected {MANIFEST_SCHEMA})",
                self.schema
            ));
        }
        for (field, text) in [
            ("version", &self.version),
            ("description", &self.description),
        ] {
            if text.trim().is_empty() || text.len() > MAX_TEXT || text.chars().any(char::is_control)
            {
                return bad(format!("{field} must be one line of 1 to {MAX_TEXT} bytes"));
            }
        }
        let mut seen = HashSet::new();
        if let Some(dup) = self.context.iter().find(|f| !seen.insert(**f)) {
            return bad(format!("context lists {} twice", dup.as_str()));
        }
        if !(1..=MAX_FUEL).contains(&self.limits.fuel) {
            return bad(format!("limits.fuel must be 1 to {MAX_FUEL}"));
        }
        if !(1..=MAX_MEMORY_MIB).contains(&self.limits.memory_mib) {
            return bad(format!("limits.memory_mib must be 1 to {MAX_MEMORY_MIB}"));
        }
        Ok(())
    }

    /// The consent text: one plain sentence per permission, the sandbox
    /// guarantee last.
    #[must_use]
    pub fn permissions(&self) -> Vec<String> {
        std::iter::once(
            "It may change the content of the files of the items that use it, before they are \
             restored."
                .to_owned(),
        )
        .chain(self.context.iter().map(|f| f.permission().to_owned()))
        .chain(std::iter::once(
            "It cannot read or write other files, use the network or run programs.".to_owned(),
        ))
        .collect()
    }
}
