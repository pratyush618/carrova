//! Neighbour (ARP) table lookups, for Wake-on-LAN hints: pure parsers over
//! the text each OS offers, so every format is testable on any host.

use std::net::Ipv4Addr;

/// MAC of `ip` in `/proc/net/arp`-formatted `table`: the first complete
/// entry (flag `0x2`, usable address). Malformed lines are skipped.
#[cfg(any(test, target_os = "linux"))]
pub(crate) fn mac_from_proc_arp(table: &str, ip: Ipv4Addr) -> Option<[u8; 6]> {
    table.lines().find_map(|line| {
        let mut f = line.split_whitespace();
        let (entry_ip, _hw_type, flags, mac) = (f.next()?, f.next()?, f.next()?, f.next()?);
        if entry_ip.parse::<Ipv4Addr>().ok()? != ip {
            return None;
        }
        let complete = u32::from_str_radix(flags.strip_prefix("0x")?, 16).ok()? & 0x2 != 0;
        complete.then(|| usable(parse_mac(mac, ':')?)).flatten()
    })
}

/// MAC of `ip` in macOS `arp -an` output, e.g.
/// `? (192.168.1.1) at 0:1b:2c:d:e:ff on en0 ifscope [ethernet]` (bytes
/// lose leading zeros; `(incomplete)` entries have no address).
#[cfg(any(test, target_os = "macos"))]
pub(crate) fn mac_from_bsd_arp(output: &str, ip: Ipv4Addr) -> Option<[u8; 6]> {
    output.lines().find_map(|line| {
        let mut f = line.split_whitespace();
        let (_name, entry_ip, at, mac) = (f.next()?, f.next()?, f.next()?, f.next()?);
        let entry_ip = entry_ip.strip_prefix('(')?.strip_suffix(')')?;
        if at != "at" || entry_ip.parse::<Ipv4Addr>().ok()? != ip {
            return None;
        }
        usable(parse_mac(mac, ':')?)
    })
}

/// MAC of `ip` in Windows `arp -a` output: rows of address, dash-separated
/// MAC and type. Headers (localised) and anything else are skipped.
#[cfg(any(test, target_os = "windows"))]
pub(crate) fn mac_from_windows_arp(output: &str, ip: Ipv4Addr) -> Option<[u8; 6]> {
    output.lines().find_map(|line| {
        let mut f = line.split_whitespace();
        let (entry_ip, mac, _kind) = (f.next()?, f.next()?, f.next()?);
        if entry_ip.parse::<Ipv4Addr>().ok()? != ip {
            return None;
        }
        usable(parse_mac(mac, '-')?)
    })
}

/// `mac` unless it is the unset (all-zero) or the broadcast address.
fn usable(mac: [u8; 6]) -> Option<[u8; 6]> {
    (mac != [0; 6] && mac != [0xff; 6]).then_some(mac)
}

fn parse_mac(text: &str, sep: char) -> Option<[u8; 6]> {
    let mut mac = [0u8; 6];
    let mut parts = text.split(sep);
    for byte in &mut mac {
        *byte = u8::from_str_radix(parts.next()?, 16).ok()?;
    }
    parts.next().is_none().then_some(mac)
}

#[cfg(test)]
#[path = "../tests/neighbours/unit.rs"]
mod tests;
