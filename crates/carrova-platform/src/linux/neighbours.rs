//! Kernel neighbour (ARP) table lookups, for Wake-on-LAN hints.

use std::net::Ipv4Addr;

/// MAC of `ip` in `/proc/net/arp`-formatted `table`: the first complete
/// entry (flag `0x2`, non-zero address). Malformed lines are skipped.
pub(super) fn mac_from_arp(table: &str, ip: Ipv4Addr) -> Option<[u8; 6]> {
    table.lines().find_map(|line| {
        let mut f = line.split_whitespace();
        let (entry_ip, _hw_type, flags, mac) = (f.next()?, f.next()?, f.next()?, f.next()?);
        if entry_ip.parse::<Ipv4Addr>().ok()? != ip {
            return None;
        }
        let complete = u32::from_str_radix(flags.strip_prefix("0x")?, 16).ok()? & 0x2 != 0;
        let mac = parse_mac(mac)?;
        (complete && mac != [0; 6]).then_some(mac)
    })
}

fn parse_mac(text: &str) -> Option<[u8; 6]> {
    let mut mac = [0u8; 6];
    let mut parts = text.split(':');
    for byte in &mut mac {
        *byte = u8::from_str_radix(parts.next()?, 16).ok()?;
    }
    parts.next().is_none().then_some(mac)
}

#[cfg(test)]
#[path = "../../tests/linux/neighbours/unit.rs"]
mod tests;
