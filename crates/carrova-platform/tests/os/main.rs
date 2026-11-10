//! OS identity: stable ids, parsing and the permission model per OS.

use carrova_platform::{OsKind, PermissionModel, UnknownOsKind};

#[test]
fn os_kind_ids_and_display_names_are_stable() {
    let cases = [
        (OsKind::Linux, "linux", "Linux"),
        (OsKind::MacOs, "macos", "macOS"),
        (OsKind::Windows, "windows", "Windows"),
    ];
    for (os, id, name) in cases {
        assert_eq!(os.as_str(), id);
        assert_eq!(os.to_string(), name);
    }
}

#[test]
fn os_kind_parses_back_from_every_stable_id() {
    for os in OsKind::ALL {
        assert_eq!(os.as_str().parse::<OsKind>(), Ok(os));
    }
}

#[test]
fn os_kind_rejects_display_names_serde_forms_and_unknown_ids() {
    for bad in ["macOS", "Linux", "mac_os", "", "freebsd", " linux"] {
        assert_eq!(
            bad.parse::<OsKind>(),
            Err(UnknownOsKind(bad.to_owned())),
            "{bad:?}"
        );
    }
}

#[test]
fn unknown_os_error_names_the_input_and_the_accepted_ids() {
    let msg = "beos".parse::<OsKind>().unwrap_err().to_string();
    assert!(msg.contains("`beos`"), "{msg}");
    assert!(msg.contains("`macos`"), "{msg}");
}

#[test]
fn only_windows_uses_acls_instead_of_mode_bits() {
    assert_eq!(OsKind::Linux.permission_model(), PermissionModel::Unix);
    assert_eq!(OsKind::MacOs.permission_model(), PermissionModel::Unix);
    assert_eq!(OsKind::Windows.permission_model(), PermissionModel::Acl);
}
