//! QUIC endpoints: UDP socket sizing, transport parameters, TLS wiring.

use std::net::{SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use carrova_platform::Platform;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{EndpointConfig, IdleTimeout, TokioRuntime, TransportConfig, VarInt};

use crate::error::{Result, SetupFailure, TransportError};
use crate::runtime::runtime;
use crate::tls::Identity;
use crate::tuning::{
    CONNECTION_RECEIVE_WINDOW, IDLE_TIMEOUT, INITIAL_RTT, KEEP_ALIVE_INTERVAL,
    MAX_CONCURRENT_BIDI_STREAMS, SEND_WINDOW, STREAM_RECEIVE_WINDOW, UDP_SOCKET_BUFFER,
};

/// Warn about clamped socket buffers once per process, not per endpoint.
static BUFFER_WARNED: AtomicBool = AtomicBool::new(false);

/// IPv4 UDP socket on `port` (0 = any) with enlarged send/receive buffers.
fn udp_socket(platform: &dyn Platform, port: u16) -> Result<UdpSocket> {
    let (socket, granted) = platform.bind_udp(port, UDP_SOCKET_BUFFER)?;
    if (granted.send < UDP_SOCKET_BUFFER || granted.recv < UDP_SOCKET_BUFFER)
        && !BUFFER_WARNED.swap(true, Ordering::Relaxed)
    {
        let hint = platform
            .udp_buffer_hint(UDP_SOCKET_BUFFER)
            .unwrap_or_default();
        tracing::warn!(
            requested = UDP_SOCKET_BUFFER,
            send = granted.send,
            recv = granted.recv,
            "UDP socket buffers capped by the OS; {hint}"
        );
    }
    Ok(socket)
}

/// QUIC transport parameters (see `tuning`).
fn transport_config() -> Result<Arc<TransportConfig>> {
    let mut t = TransportConfig::default();
    t.stream_receive_window(VarInt::from_u32(STREAM_RECEIVE_WINDOW))
        .receive_window(VarInt::from_u32(CONNECTION_RECEIVE_WINDOW))
        .send_window(SEND_WINDOW)
        .max_concurrent_bidi_streams(VarInt::from_u32(MAX_CONCURRENT_BIDI_STREAMS))
        .max_concurrent_uni_streams(VarInt::from_u32(0))
        .initial_rtt(INITIAL_RTT)
        .max_idle_timeout(Some(
            IdleTimeout::try_from(IDLE_TIMEOUT)
                .map_err(|e| TransportError::internal("idle timeout", e))?,
        ))
        .keep_alive_interval(Some(KEEP_ALIVE_INTERVAL));
    Ok(Arc::new(t))
}

fn endpoint(socket: UdpSocket, server: Option<quinn::ServerConfig>) -> Result<quinn::Endpoint> {
    let rt = runtime()?;
    let _guard = rt.enter();
    Ok(quinn::Endpoint::new(
        EndpointConfig::default(),
        server,
        socket,
        Arc::new(TokioRuntime),
    )?)
}

/// Sender endpoint on `port` (0 = any), accepting mutual-TLS connections.
pub(crate) fn server(
    platform: &dyn Platform,
    identity: &Identity,
    port: u16,
) -> Result<quinn::Endpoint> {
    let tls = QuicServerConfig::try_from(identity.server_config()?)
        .map_err(|e| TransportError::internal("QUIC server crypto", e))?;
    let mut cfg = quinn::ServerConfig::with_crypto(Arc::new(tls));
    cfg.transport_config(transport_config()?);
    endpoint(udp_socket(platform, port)?, Some(cfg))
}

/// Receiver endpoint on an ephemeral port, with its client config.
pub(crate) fn client(
    platform: &dyn Platform,
    identity: &Identity,
) -> Result<(quinn::Endpoint, quinn::ClientConfig)> {
    client_on(udp_socket(platform, 0)?, identity)
}

/// Receiver endpoint on an already bound (non-blocking) `socket`, with its
/// client config.
pub(crate) fn client_on(
    socket: UdpSocket,
    identity: &Identity,
) -> Result<(quinn::Endpoint, quinn::ClientConfig)> {
    let tls = QuicClientConfig::try_from(identity.client_config()?)
        .map_err(|e| TransportError::internal("QUIC client crypto", e))?;
    let mut cfg = quinn::ClientConfig::new(Arc::new(tls));
    cfg.transport_config(transport_config()?);
    Ok((endpoint(socket, None)?, cfg))
}

/// Local IPv4 address of an endpoint.
pub(crate) fn local_v4(endpoint: &quinn::Endpoint) -> Result<SocketAddrV4> {
    match endpoint.local_addr()? {
        SocketAddr::V4(a) => Ok(a),
        SocketAddr::V6(_) => Err(SetupFailure::NotIpv4.into()),
    }
}
