use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::linux::{LinuxEnv, LinuxPlatform};
use crate::{Platform, PlatformError, SnapshotCapability};

// ---- mountinfo / snapshots ----

const MOUNTINFO: &str = "\
22 1 0:21 / / rw,relatime shared:1 - ext4 /dev/sda1 rw
30 22 0:30 / /home rw,relatime shared:2 - btrfs /dev/sda2 rw,subvol=/home
31 30 0:31 / /home/u/My\\040Disk rw - ext4 /dev/sdb1 rw
32 22 253:0 / /data rw shared:3 master:1 - xfs /dev/mapper/vg-data rw
33 22 253:1 / /secret rw - ext4 /dev/mapper/luks-abc rw
34 22 253:2 / /dm rw - ext4 /dev/dm-2 rw
garbage line
";

#[test]
fn parse_mountinfo_fields_and_escapes() {
    let m = parse_mountinfo(MOUNTINFO);
    assert_eq!(m.len(), 6);
    assert_eq!(m[2].mount_point, PathBuf::from("/home/u/My Disk"));
    assert_eq!(m[3].fs_type, "xfs");
    assert_eq!(m[3].source, "/dev/mapper/vg-data");
    assert_eq!(m[3].dev, "253:0");
    assert_eq!(unescape_mount("a\\011b\\134c\\9zz\\04"), "a\tb\\c\\9zz\\04");
}

#[test]
fn longest_prefix_wins() {
    let m = parse_mountinfo(MOUNTINFO);
    let at = |p: &str| {
        containing_mount(&m, Path::new(p))
            .unwrap()
            .mount_point
            .clone()
    };
    assert_eq!(at("/etc/x"), PathBuf::from("/"));
    assert_eq!(at("/home/u/.config"), PathBuf::from("/home"));
    assert_eq!(at("/home/u/My Disk/f"), PathBuf::from("/home/u/My Disk"));
    // component-wise, not string prefix
    assert_eq!(at("/homework"), PathBuf::from("/"));
}

#[test]
fn stacked_mount_later_wins() {
    let m = parse_mountinfo(
        "1 0 0:1 / / rw - ext4 /dev/a rw\n2 1 0:2 / /x rw - ext4 /dev/b rw\n3 1 0:3 / /x rw - btrfs /dev/c rw\n",
    );
    assert_eq!(
        containing_mount(&m, Path::new("/x/y")).unwrap().fs_type,
        "btrfs"
    );
}

/// Mountinfo that maps the tempdir's canonical path to the given entry.
fn snapshot_for(fs_type: &str, source: &str, dm_uuid: Option<&str>) -> SnapshotCapability {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("target");
    fs::create_dir_all(target.join("sub")).unwrap();
    let canon = fs::canonicalize(&target).unwrap();
    let mp = canon.to_str().unwrap().replace(' ', "\\040");
    let info = format!(
        "1 0 0:1 / / rw - ext4 /dev/sda1 rw\n9 1 253:7 / {mp} rw shared:1 - {fs_type} {source} rw\n"
    );
    let mi = tmp.path().join("mountinfo");
    fs::write(&mi, info).unwrap();
    let sys = tmp.path().join("sys");
    if let Some(uuid) = dm_uuid {
        let d = sys.join("dev/block/253:7/dm");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("uuid"), format!("{uuid}\n")).unwrap();
    }
    LinuxPlatform::new(LinuxEnv::default())
        .with_mountinfo(mi)
        .with_sys_root(sys)
        .snapshot_capability(&target.join("sub"))
        .unwrap()
}

#[test]
fn snapshot_capability_detection() {
    assert_eq!(
        snapshot_for("btrfs", "/dev/sda2", None),
        SnapshotCapability::Btrfs
    );
    assert_eq!(
        snapshot_for("ext4", "/dev/sdb1", None),
        SnapshotCapability::None
    );
    assert_eq!(
        snapshot_for("ext4", "/dev/mapper/vg-root", Some("LVM-abc")),
        SnapshotCapability::Lvm
    );
    assert_eq!(
        snapshot_for("ext4", "/dev/dm-0", Some("LVM-abc")),
        SnapshotCapability::Lvm
    );
    assert_eq!(
        snapshot_for("ext4", "/dev/mapper/luks-1", Some("CRYPT-LUKS2-abc")),
        SnapshotCapability::None
    );
    // sysfs unreadable → best-effort guess from device name
    assert_eq!(
        snapshot_for("xfs", "/dev/mapper/vg-data", None),
        SnapshotCapability::Lvm
    );
}

#[test]
fn snapshot_capability_missing_path_is_error() {
    let p = LinuxPlatform::new(LinuxEnv::default());
    assert!(matches!(
        p.snapshot_capability(Path::new("/definitely/not/here")),
        Err(PlatformError::Io { .. })
    ));
}

#[test]
fn snapshot_capability_real_root_smoke() {
    let p = LinuxPlatform::from_env();
    assert!(p.snapshot_capability(Path::new("/")).is_ok());
}
