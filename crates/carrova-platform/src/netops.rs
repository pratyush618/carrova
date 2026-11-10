//! LAN networking primitives (Linux, macOS, Windows) and Unix process
//! identity, shared by the native adapters and the fake (real in all).

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
/// shares the port with other such sockets, each receiving every
/// broadcast: `SO_REUSEADDR` on Linux and Windows, `SO_REUSEPORT` on
/// macOS/BSD (where `SO_REUSEADDR` shares only multicast binds).
///
/// Security: sharing means another local process can bind the same port
/// and read (or answer) discovery broadcasts; Windows `SO_EXCLUSIVEADDRUSE`
/// would forbid exactly the sharing we need. Discovery data is public by design;
/// pairing and transfer are authenticated end to end, never trusted from
/// this socket.
pub(crate) fn bind_broadcast_udp(port: u16) -> io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    #[cfg(any(target_os = "linux", windows))]
    socket.set_reuse_address(true)?;
    #[cfg(all(unix, not(target_os = "linux")))]
    socket.set_reuse_port(true)?;
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&any_v4(port))?;
    Ok(socket.into())
}

/// Smallest buffer request [`bind_udp`] steps down to before giving up.
const MIN_UDP_BUFFER: usize = 64 * 1024;

/// Non-blocking IPv4 UDP socket on `port` with `SO_SNDBUF`/`SO_RCVBUF`
/// requested at `buffer` bytes. Linux clamps requests above
/// `net.core.{w,r}mem_max`; macOS refuses them (`ENOBUFS` above
/// `kern.ipc.maxsockbuf`), so a refused request is halved and retried.
/// The read-back reports what was granted.
pub(crate) fn bind_udp(port: u16, buffer: usize) -> io::Result<(UdpSocket, SocketBuffers)> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    request_buffer(buffer, |n| socket.set_send_buffer_size(n))?;
    request_buffer(buffer, |n| socket.set_recv_buffer_size(n))?;
    let buffers = SocketBuffers {
        send: socket.send_buffer_size()?,
        recv: socket.recv_buffer_size()?,
    };
    socket.set_nonblocking(true)?;
    socket.bind(&any_v4(port))?;
    Ok((socket.into(), buffers))
}

/// Ask for `wanted` bytes via `set`, halving on refusal down to
/// [`MIN_UDP_BUFFER`]; the last refusal is returned.
fn request_buffer(wanted: usize, set: impl Fn(usize) -> io::Result<()>) -> io::Result<()> {
    let mut size = wanted;
    loop {
        match set(size) {
            Err(_) if size > MIN_UDP_BUFFER => size = (size / 2).max(MIN_UDP_BUFFER),
            result => return result,
        }
    }
}

/// Effective uid of this process.
#[cfg(unix)]
pub(crate) fn current_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// The kernel host name (`uname` nodename): what `hostname` prints and
/// what the LAN knows this machine as. Unlike `/etc/hostname` it is
/// always set and reflects transient (DHCP, `hostnamectl`) names.
#[cfg(unix)]
pub(crate) fn host_name() -> String {
    rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .into_owned()
}

/// How to raise the UDP buffer limit to `wanted` bytes (Linux sysctl;
/// macOS has its own hint, the fake reuses this one).
#[cfg(any(target_os = "linux", all(unix, feature = "fake")))]
pub(crate) fn udp_buffer_hint(wanted: usize) -> String {
    format!(
        "raise the limit for full LAN speed, e.g. \
         `sudo sysctl -w net.core.rmem_max={wanted} net.core.wmem_max={wanted}`"
    )
}

#[cfg(test)]
#[path = "../tests/netops/unit.rs"]
mod tests;
