use super::*;

const TABLE: &str = "\
IP address       HW type     Flags       HW address            Mask     Device
192.168.1.1      0x1         0x2         aa:bb:cc:dd:ee:ff     *        eth0
192.168.1.7      0x1         0x0         00:00:00:00:00:00     *        eth0
192.168.1.8      0x1         0x0         11:22:33:44:55:66     *        eth0
192.168.1.9      0x1         0x2         00:00:00:00:00:00     *        eth0
10.0.0.5         0x1         0x2         01:02:03:04:05:06     *        wlan0
10.0.0.6         0x1         0x2         not-a-mac             *        wlan0
10.0.0.7         0x1
garbage
";

fn lookup(ip: &str) -> Option<[u8; 6]> {
    mac_from_arp(TABLE, ip.parse().unwrap())
}

#[test]
fn complete_entry_yields_its_mac() {
    assert_eq!(
        lookup("192.168.1.1"),
        Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff])
    );
}

#[test]
fn entry_on_another_interface_is_found() {
    assert_eq!(lookup("10.0.0.5"), Some([1, 2, 3, 4, 5, 6]));
}

#[test]
fn incomplete_entries_are_skipped() {
    assert_eq!(lookup("192.168.1.7"), None);
    assert_eq!(lookup("192.168.1.8"), None);
    assert_eq!(lookup("192.168.1.9"), None);
}

#[test]
fn malformed_lines_and_unknown_hosts_are_skipped() {
    assert_eq!(lookup("10.0.0.6"), None);
    assert_eq!(lookup("10.0.0.7"), None);
    assert_eq!(lookup("172.16.0.1"), None);
}
