//! Linux process-identity and LAN networking primitives shared by the
//! Linux adapter and the fake (real in both).

use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

use socket2::{Domain, Protocol, Socket, Type};

use crate::{LanInterface, PlatformError, SocketBuffers};

/// Usable LAN IPv4 interfaces: up (or state unknown), not loopback, not
/// link-local.
pub(crate) fn lan_ipv4_interfaces() -> Result<Vec<LanInterface>, PlatformError> {
    let all = if_addrs::get_if_addrs().map_err(|e| PlatformError::Network(e.to_string()))?;
    Ok(all
        .into_iter()
        .filter(|i| {
            matches!(
                i.oper_status,
                if_addrs::IfOperStatus::Up | if_addrs::IfOperStatus::Unknown
            )
        })
        .filter_map(|i| match i.addr {
            if_addrs::IfAddr::V4(a) if !a.ip.is_loopback() && !a.ip.is_link_local() => {
                Some(LanInterface {
                    ip: a.ip,
                    broadcast: a.broadcast,
                })
            }
            _ => None,
        })
        .collect())
}

fn any_v4(port: u16) -> socket2::SockAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).into()
}

/// Non-blocking IPv4 UDP socket on `port` that may send broadcasts and
/// shares the port with other such sockets: on Linux `SO_REUSEADDR` lets
/// several UDP sockets bind one port, and a broadcast reaches every one.
pub(crate) fn bind_broadcast_udp(port: u16) -> io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&any_v4(port))?;
    Ok(socket.into())
}

/// Non-blocking IPv4 UDP socket on `port` with `SO_SNDBUF`/`SO_RCVBUF`
/// requested at `buffer` bytes. Linux clamps requests above
/// `net.core.{w,r}mem_max` instead of refusing them; the read-back reports
/// what was granted.
pub(crate) fn bind_udp(port: u16, buffer: usize) -> io::Result<(UdpSocket, SocketBuffers)> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_send_buffer_size(buffer)?;
    socket.set_recv_buffer_size(buffer)?;
    let buffers = SocketBuffers {
        send: socket.send_buffer_size()?,
        recv: socket.recv_buffer_size()?,
    };
    socket.set_nonblocking(true)?;
    socket.bind(&any_v4(port))?;
    Ok((socket.into(), buffers))
}

/// Effective uid of this process.
pub(crate) fn current_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// The kernel host name (`uname` nodename): what `hostname` prints and
/// what the LAN knows this machine as. Unlike `/etc/hostname` it is
/// always set and reflects transient (DHCP, `hostnamectl`) names.
pub(crate) fn host_name() -> String {
    rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .into_owned()
}

/// How to raise the UDP buffer limit to `wanted` bytes.
pub(crate) fn udp_buffer_hint(wanted: usize) -> String {
    format!(
        "raise the limit for full LAN speed, e.g. \
         `sudo sysctl -w net.core.rmem_max={wanted} net.core.wmem_max={wanted}`"
    )
}

#[cfg(test)]
#[path = "../tests/netops/unit.rs"]
mod tests;
