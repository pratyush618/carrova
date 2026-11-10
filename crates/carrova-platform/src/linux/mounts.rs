//! `/proc/self/mountinfo` parsing and snapshot-capability detection.

use std::path::{Path, PathBuf};

use super::LinuxPlatform;
use crate::{PlatformError, SnapshotCapability};

impl LinuxPlatform {
    /// Best-effort detection:
    /// - fs type `btrfs` → [`SnapshotCapability::Btrfs`].
    /// - device-mapper source (`/dev/mapper/*`, `/dev/dm-*`) → checks
    ///   `/sys/dev/block/<maj:min>/dm/uuid`: `LVM-*` → Lvm, any other uuid
    ///   (e.g. `CRYPT-*` for LUKS) → None. If sysfs is unreadable, assumes Lvm.
    /// - otherwise None.
    ///
    /// LUKS-on-LVM reports None (the top dm device is the crypt mapping); the
    /// caller loses only an optimisation, never correctness.
    pub(super) fn detect_snapshot_capability(
        &self,
        path: &Path,
    ) -> Result<SnapshotCapability, PlatformError> {
        let canonical = std::fs::canonicalize(path).map_err(|e| PlatformError::io(path, e))?;
        let text = std::fs::read_to_string(&self.mountinfo)
            .map_err(|e| PlatformError::io(&self.mountinfo, e))?;
        let mounts = parse_mountinfo(&text);
        let Some(mount) = containing_mount(&mounts, &canonical) else {
            return Ok(SnapshotCapability::None);
        };
        if mount.fs_type == "btrfs" {
            return Ok(SnapshotCapability::Btrfs);
        }
        let is_dm =
            mount.source.starts_with("/dev/mapper/") || mount.source.starts_with("/dev/dm-");
        if !is_dm {
            return Ok(SnapshotCapability::None);
        }
        let uuid_path = self
            .sys_root
            .join("dev/block")
            .join(&mount.dev)
            .join("dm/uuid");
        Ok(match std::fs::read_to_string(uuid_path) {
            Ok(uuid) if uuid.trim().starts_with("LVM-") => SnapshotCapability::Lvm,
            Ok(_) => SnapshotCapability::None,
            Err(_) => SnapshotCapability::Lvm,
        })
    }
}

/// A parsed `/proc/self/mountinfo` line (only the fields we use).
#[derive(Debug, Clone, PartialEq, Eq)]
struct MountEntry {
    dev: String,
    mount_point: PathBuf,
    fs_type: String,
    source: String,
}

/// Decode mountinfo octal escapes (`\040` space, `\011` tab, `\012` newline,
/// `\134` backslash). Malformed escapes are kept literally.
fn unescape_mount(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 4 <= bytes.len() {
            let oct = &bytes[i + 1..i + 4];
            if oct.iter().all(|b| (b'0'..=b'7').contains(b)) {
                let v = u32::from(oct[0] - b'0') * 64
                    + u32::from(oct[1] - b'0') * 8
                    + u32::from(oct[2] - b'0');
                if let Ok(b) = u8::try_from(v) {
                    out.push(b);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse mountinfo; malformed lines are skipped.
///
/// Format: `id parent maj:min root mount_point opts [optional...] - fstype source superopts`
fn parse_mountinfo(text: &str) -> Vec<MountEntry> {
    text.lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let sep = fields.iter().position(|f| *f == "-")?;
            if sep < 6 {
                return None;
            }
            let fs_type = fields.get(sep + 1)?;
            let source = fields.get(sep + 2).copied().unwrap_or("");
            Some(MountEntry {
                dev: fields[2].to_owned(),
                mount_point: PathBuf::from(unescape_mount(fields[4])),
                fs_type: unescape_mount(fs_type),
                source: unescape_mount(source),
            })
        })
        .collect()
}

/// Mount whose mount point is the longest component-wise prefix of `path`.
/// On ties (stacked mounts) the later entry wins, as it shadows earlier ones.
fn containing_mount<'a>(mounts: &'a [MountEntry], path: &Path) -> Option<&'a MountEntry> {
    let mut best: Option<&MountEntry> = None;
    for m in mounts {
        if path.starts_with(&m.mount_point) {
            let len = m.mount_point.components().count();
            if best.is_none_or(|b| len >= b.mount_point.components().count()) {
                best = Some(m);
            }
        }
    }
    best
}

#[cfg(test)]
#[path = "../../tests/linux/mounts/unit.rs"]
mod tests;
