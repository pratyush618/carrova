use std::ffi::OsStr;
use std::path::Prefix;

use super::is_supported_prefix;

#[test]
fn drive_and_unc_prefixes_are_supported() {
    assert!(is_supported_prefix(Prefix::Disk(b'C')));
    assert!(is_supported_prefix(Prefix::UNC(
        OsStr::new("server"),
        OsStr::new("share")
    )));
}

#[test]
fn verbatim_and_device_prefixes_are_refused() {
    let refused = [
        Prefix::Verbatim(OsStr::new("x")),
        Prefix::VerbatimDisk(b'C'),
        Prefix::VerbatimUNC(OsStr::new("server"), OsStr::new("share")),
        Prefix::DeviceNS(OsStr::new("pipe")),
    ];
    for prefix in refused {
        assert!(!is_supported_prefix(prefix), "{prefix:?}");
    }
}
