//! Raw QUIC peers that speak handshake frames by hand, from any loopback
//! source address (Linux routes all of 127/8 locally), to play receivers
//! that stall, replay or pause mid-handshake.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::Duration;

use carrova_crypto::pairing::{Confirmation, PairingCode, Pake, PakeMessage, PakeRole};
use quinn::{ConnectionError, SendStream, TransportErrorCode};
use tokio::time::timeout;

use crate::PROTOCOL_VERSION;
use crate::endpoint;
use crate::handshake::binding;
use crate::runtime::runtime;
use crate::tls::Identity;
use crate::tuning::TLS_SERVER_NAME;
use crate::wire::{CloseCode, Handshake, Intent, RejectReason, StreamReader, encode_frame};

/// `127.0.0.<n>`.
pub(super) fn loopback(n: u8) -> Ipv4Addr {
    Ipv4Addr::new(127, 0, 0, n)
}

/// How a raw dial ended.
#[derive(Debug)]
pub(super) enum Dial {
    /// QUIC/TLS handshake completed.
    Connected(Box<RawPeer>),
    /// The sender refused the connection (`CONNECTION_REFUSED`).
    Refused,
    /// Anything else (including no answer within the wait); the reason
    /// shows in assertion messages.
    Failed(#[expect(dead_code, reason = "read through Debug in assertion messages")] String),
}

impl Dial {
    pub(super) fn connected(self) -> RawPeer {
        match self {
            Self::Connected(peer) => *peer,
            other => panic!("expected a connection, got {other:?}"),
        }
    }
}

/// One unauthenticated QUIC connection to a sender.
#[derive(Debug)]
pub(super) struct RawPeer {
    _endpoint: quinn::Endpoint,
    identity: Identity,
    pub(super) conn: quinn::Connection,
    stream: Option<(SendStream, StreamReader)>,
}

/// Dial the sender on `port` from `source`, waiting at most `wait`.
pub(super) fn dial(source: Ipv4Addr, port: u16, wait: Duration) -> Dial {
    let socket = UdpSocket::bind(SocketAddrV4::new(source, 0)).unwrap();
    socket.set_nonblocking(true).unwrap();
    let identity = Identity::generate().unwrap();
    let rt = runtime().unwrap();
    let (ep, cfg) = endpoint::client_on(socket, &identity).unwrap();
    // Dial the sender at `source` itself, not 127.0.0.1: the reply then
    // comes from the address dialed on every OS (macOS would answer a
    // 127.0.0.1 dial from the alias, which QUIC drops as a path change).
    let addr = SocketAddr::V4(SocketAddrV4::new(source, port));
    let connected = rt.block_on(async {
        let connecting = ep.connect_with(cfg, addr, TLS_SERVER_NAME).unwrap();
        timeout(wait, connecting).await
    });
    match connected {
        Ok(Ok(conn)) => Dial::Connected(Box::new(RawPeer {
            _endpoint: ep,
            identity,
            conn,
            stream: None,
        })),
        Ok(Err(ConnectionError::ConnectionClosed(close)))
            if close.error_code == TransportErrorCode::CONNECTION_REFUSED =>
        {
            Dial::Refused
        }
        Ok(Err(e)) => Dial::Failed(e.to_string()),
        Err(_) => Dial::Failed("no answer".into()),
    }
}

/// `n` connections that complete TLS and then say nothing, each from its
/// own source address starting at `127.0.0.<first>`.
pub(super) fn stalled(port: u16, first: u8, n: usize) -> Vec<RawPeer> {
    (first..)
        .take(n)
        .map(|i| dial(loopback(i), port, Duration::from_secs(5)).connected())
        .collect()
}

impl RawPeer {
    /// Send `msg` on the control stream (opened on first use).
    pub(super) fn send(&mut self, msg: &Handshake) {
        let rt = runtime().unwrap();
        if self.stream.is_none() {
            let (send, recv) = rt.block_on(self.conn.open_bi()).unwrap();
            self.stream = Some((send, StreamReader::new(recv)));
        }
        let (send, _) = self.stream.as_mut().unwrap();
        rt.block_on(send.write_all(&encode_frame(msg).unwrap()))
            .unwrap();
    }

    /// Next frame from the sender.
    pub(super) fn recv(&mut self) -> crate::Result<Handshake> {
        let (_, recv) = self.stream.as_mut().unwrap();
        runtime().unwrap().block_on(recv.expect_frame())
    }

    /// Send `Hello{intent}` and return the sender's answer.
    pub(super) fn hello(&mut self, intent: Intent) -> crate::Result<Handshake> {
        self.send(&Handshake::Hello {
            proto: PROTOCOL_VERSION,
            intent,
        });
        self.recv()
    }

    /// Whether the sender turned this `Hello` away: a `Reject` frame, or
    /// (the frame is best effort) the `Rejected` close.
    pub(super) fn turned_away(&self, answer: &crate::Result<Handshake>) -> bool {
        match answer {
            Ok(Handshake::Reject(RejectReason::NotOffered)) => true,
            Ok(_) => false,
            Err(_) => matches!(
                self.conn.close_reason(),
                Some(ConnectionError::ApplicationClosed(c))
                    if CloseCode::from_wire(c.error_code) == Some(CloseCode::Rejected)
            ),
        }
    }

    /// Receiver side of pairing, split around the wait the caller wants:
    /// returns the PAKE state after sending the receiver's message.
    pub(super) fn send_pake(&mut self, code: &PairingCode) -> Pake {
        let (pake, msg) = Pake::start(code, PakeRole::Receiver);
        self.send(&Handshake::Pake(msg.into_bytes()));
        pake
    }

    /// Finish pairing after [`Self::send_pake`]: check the sender's
    /// confirmation, send ours, and wait for `Established`.
    pub(super) fn finish_pairing(&mut self, pake: Pake) -> crate::Result<()> {
        let Handshake::Pake(peer) = self.recv()? else {
            return Err(crate::ProtocolViolation::UnexpectedHandshake { wanted: "PAKE" }.into());
        };
        let keys = pake.finish(&PakeMessage::from_bytes(peer)).unwrap();
        let (binding, _) = binding(&self.conn, &self.identity, PakeRole::Receiver)?;
        let Handshake::Confirm(theirs) = self.recv()? else {
            return Err(crate::ProtocolViolation::UnexpectedHandshake { wanted: "confirm" }.into());
        };
        keys.verify(PakeRole::Sender, &binding, &Confirmation(theirs))
            .map_err(|_| crate::TransportError::PairingFailed)?;
        let ours = keys.confirm(PakeRole::Receiver, &binding).unwrap();
        self.send(&Handshake::Confirm(ours.0));
        match self.recv()? {
            Handshake::Established => Ok(()),
            _ => Err(crate::ProtocolViolation::UnexpectedHandshake {
                wanted: "established",
            }
            .into()),
        }
    }
}
