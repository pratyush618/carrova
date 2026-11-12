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
    mac_from_proc_arp(TABLE, ip.parse().unwrap())
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

const BSD_ARP: &str = "\
? (192.168.1.1) at 0:1b:2c:d:e:ff on en0 ifscope [ethernet]
? (192.168.1.20) at (incomplete) on en0 ifscope [ethernet]
? (192.168.1.21) at 0:0:0:0:0:0 on en0 ifscope [ethernet]
? (192.168.1.22) at ff:ff:ff:ff:ff:ff on en0 ifscope [ethernet]
? (10.0.0.5) at 1:2:3:4:5:6 on en1 ifscope [ethernet]
? (10.0.0.6) at 1:2:3:4:5 on en1 ifscope [ethernet]
? (10.0.0.7)
mDNS garbage
";

const WINDOWS_ARP: &str = "\
Interface: 192.168.1.50 --- 0x4
  Internet Address      Physical Address      Type
  192.168.1.1           aa-bb-cc-dd-ee-ff     dynamic
  192.168.1.21          00-00-00-00-00-00     invalid
  192.168.1.255         ff-ff-ff-ff-ff-ff     static
  224.0.0.22            01-00-5e-00-00-16     static

Schnittstelle: 10.0.0.2 --- 0x9
  Internetadresse       Physische Adresse     Typ
  10.0.0.5              01-02-03-04-05-06     dynamisch
  10.0.0.6              01:02:03:04:05:06     dynamisch
  10.0.0.7              not-a-mac             dynamisch
";

fn bsd(ip: &str) -> Option<[u8; 6]> {
    mac_from_bsd_arp(BSD_ARP, ip.parse().unwrap())
}

fn windows(ip: &str) -> Option<[u8; 6]> {
    mac_from_windows_arp(WINDOWS_ARP, ip.parse().unwrap())
}

#[test]
fn bsd_entry_with_dropped_leading_zeros_yields_its_mac() {
    assert_eq!(bsd("192.168.1.1"), Some([0, 0x1b, 0x2c, 0xd, 0xe, 0xff]));
    assert_eq!(bsd("10.0.0.5"), Some([1, 2, 3, 4, 5, 6]));
}

#[test]
fn bsd_incomplete_zero_broadcast_and_malformed_entries_are_skipped() {
    for ip in [
        "192.168.1.20",
        "192.168.1.21",
        "192.168.1.22",
        "10.0.0.6",
        "10.0.0.7",
        "172.16.0.1",
    ] {
        assert_eq!(bsd(ip), None, "{ip}");
    }
}

#[test]
fn windows_entry_yields_its_mac_whatever_the_header_language() {
    assert_eq!(
        windows("192.168.1.1"),
        Some([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff])
    );
    assert_eq!(windows("10.0.0.5"), Some([1, 2, 3, 4, 5, 6]));
}

#[test]
fn windows_zero_broadcast_and_malformed_entries_are_skipped() {
    for ip in [
        "192.168.1.21",
        "192.168.1.255",
        "10.0.0.6",
        "10.0.0.7",
        "172.16.0.1",
    ] {
        assert_eq!(windows(ip), None, "{ip}");
    }
}
