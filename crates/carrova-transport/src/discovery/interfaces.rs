//! Local interfaces and addresses for discovery, and which answers are
//! plausible senders.

use std::net::{IpAddr, Ipv4Addr, SocketAddrV4, UdpSocket};

use carrova_platform::Platform;

use crate::tuning::MIN_SENDER_PORT;

/// Destination used to ask the OS which local address routes to the LAN
/// (TEST-NET-1; connecting a UDP socket sends nothing).
const ROUTE_PROBE: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 9);
/// Loopback-subnet broadcast: reaches senders on this host.
const LOOPBACK_BROADCAST: Ipv4Addr = Ipv4Addr::new(127, 255, 255, 255);

/// Local address the OS routes LAN traffic from, if there is a route.
fn routed_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(ROUTE_PROBE).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) => Some(ip),
        IpAddr::V6(_) => None,
    }
}

/// LAN interfaces, empty (logged) if the platform cannot list them.
fn lan_interfaces(platform: &dyn Platform) -> Vec<carrova_platform::LanInterface> {
    platform
        .lan_ipv4_interfaces()
        .inspect_err(|e| tracing::debug!(error = %e, "cannot list LAN interfaces"))
        .unwrap_or_default()
}

/// The primary LAN IPv4 address: the routed interface if it is a LAN
/// interface, else the first private-range one, else any.
pub(crate) fn primary_lan_ipv4(platform: &dyn Platform) -> Option<Ipv4Addr> {
    let ifs = lan_interfaces(platform);
    let routed = routed_ipv4();
    ifs.iter()
        .find(|i| Some(i.ip) == routed)
        .or_else(|| ifs.iter().find(|i| i.ip.is_private()))
        .or_else(|| ifs.first())
        .map(|i| i.ip)
}

/// Where broadcast queries go: limited broadcast, this host, and every
/// interface's directed broadcast.
pub(super) fn broadcast_targets(platform: &dyn Platform) -> Vec<Ipv4Addr> {
    let mut targets = vec![Ipv4Addr::BROADCAST, LOOPBACK_BROADCAST];
    for i in lan_interfaces(platform) {
        if let Some(b) = i.broadcast
            && !targets.contains(&b)
        {
            targets.push(b);
        }
    }
    targets
}

/// Whether a discovery answer naming `addr` may be a Carrova sender: a
/// unicast address, not a privileged port, and loopback only when the
/// query was sent to loopback (`loopback_queried`).
pub(super) fn plausible_sender(addr: SocketAddrV4, loopback_queried: bool) -> bool {
    let ip = addr.ip();
    addr.port() >= MIN_SENDER_PORT
        && !ip.is_unspecified()
        && !ip.is_broadcast()
        && !ip.is_multicast()
        && (!ip.is_loopback() || loopback_queried)
}

#[cfg(test)]
#[path = "../../tests/discovery/interfaces/unit.rs"]
mod tests;
