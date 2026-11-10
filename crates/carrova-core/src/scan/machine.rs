//! Facts about the scanned machine: OS, semantic roots, free space and
//! snapshot support.

use std::collections::BTreeMap;
use std::path::PathBuf;

use carrova_platform::{Platform, SemanticRoot, SnapshotCapability};

use super::{PlatformReport, ScanError, Snapshot};

/// The machine report plus every semantic root that resolved.
pub(super) struct Machine {
    pub(super) report: PlatformReport,
    pub(super) roots: BTreeMap<SemanticRoot, PathBuf>,
}

/// Survey `platform`; unavailable roots and unknown facts become warnings.
/// Only a missing home directory is fatal.
pub(super) fn survey(
    platform: &dyn Platform,
    warnings: &mut Vec<String>,
) -> Result<Machine, ScanError> {
    let home = platform
        .resolve_root(SemanticRoot::Home)
        .map_err(ScanError::HomeUnavailable)?;
    let mut roots = BTreeMap::new();
    for root in SemanticRoot::ALL {
        match platform.resolve_root(root) {
            Ok(p) => {
                roots.insert(root, p);
            }
            Err(e) => warnings.push(format!("semantic root `{root}` unavailable: {e}")),
        }
    }
    let report = PlatformReport {
        os: platform.os(),
        roots: roots
            .iter()
            .map(|(r, p)| (*r, p.to_string_lossy().into_owned()))
            .collect(),
        free_space: platform
            .free_space(&home)
            .map_err(|e| warnings.push(format!("free space unknown: {e}")))
            .ok(),
        snapshot: platform
            .snapshot_capability(&home)
            .map_err(|e| warnings.push(format!("snapshot capability unknown: {e}")))
            .ok()
            .map(|s| match s {
                SnapshotCapability::None => Snapshot::None,
                SnapshotCapability::Btrfs => Snapshot::Btrfs,
                SnapshotCapability::Lvm => Snapshot::Lvm,
            }),
    };
    Ok(Machine { report, roots })
}
