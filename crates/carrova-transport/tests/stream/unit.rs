use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use super::*;
use crate::endpoint;
use crate::runtime::runtime;
use crate::tls::Identity;
use crate::tuning::TLS_SERVER_NAME;

/// A raw QUIC connection pair over loopback (no Carrova handshake):
/// (client, server); the endpoints must outlive them.
async fn raw_pair() -> (quinn::Connection, quinn::Connection, [quinn::Endpoint; 2]) {
    let platform = carrova_platform::current();
    let config = endpoint::server_config(
        &Identity::generate().unwrap(),
        endpoint::session_transport().unwrap(),
    )
    .unwrap();
    let server_ep = endpoint::server(platform.as_ref(), config, 0).unwrap();
    let port = endpoint::local_v4(&server_ep).unwrap().port();
    let (client_ep, cfg) =
        endpoint::client(platform.as_ref(), &Identity::generate().unwrap()).unwrap();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let connecting = client_ep.connect_with(cfg, addr, TLS_SERVER_NAME).unwrap();
    let (client, server) = tokio::join!(connecting, async {
        server_ep.accept().await.unwrap().await
    });
    (client.unwrap(), server.unwrap(), [client_ep, server_ep])
}

#[test]
fn tagless_streams_cannot_hold_accept_past_its_deadline() {
    let rt = runtime().unwrap();
    rt.block_on(async {
        let (client, server, _endpoints) = raw_pair().await;
        // Each stream sends one byte of a frame header, then nothing.
        let mut held = Vec::new();
        for _ in 0..3 {
            let (mut send, recv) = client.open_bi().await.unwrap();
            send.write_all(&[0]).await.unwrap();
            held.push((send, recv));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;

        let start = Instant::now();
        let accepted = accept(
            &server,
            &Watch::default(),
            start + Duration::from_millis(100),
        )
        .await;
        let waited = start.elapsed();
        assert!(
            matches!(accepted, Err(TransportError::Timeout)),
            "{accepted:?}"
        );
        // One tag wait may overrun the deadline; a second must not start.
        assert!(
            waited < STREAM_OPEN_TIMEOUT + Duration::from_secs(1),
            "took {waited:?}"
        );
    });
}
