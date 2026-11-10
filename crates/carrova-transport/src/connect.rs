//! Receiver side: find the sender and authenticate, yielding a
//! [`Connection`].

use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::Duration;

use carrova_crypto::pairing::ResumeSecret;
use carrova_platform::Platform;
use tokio::time::Instant;

use crate::cancel::{CancelSignal, Watch};
use crate::connection::Connection;
use crate::endpoint;
use crate::error::Result;
use crate::handshake::Secret;
use crate::race::{Dialer, race};
use crate::ticket::Ticket;
use crate::tls::Identity;

/// How the receiver connects.
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    /// OS access for sockets and interface listing (pass the engine's).
    pub platform: Arc<dyn Platform>,
    /// Stops [`connect`] and every call on the resulting connection
    /// ([`crate::TransportError::Cancelled`]); `None` = not cancellable.
    pub cancel: Option<Arc<dyn CancelSignal>>,
}

impl Default for ConnectOptions {
    /// The platform this binary was built for.
    fn default() -> Self {
        Self {
            platform: Arc::from(carrova_platform::current()),
            cancel: None,
        }
    }
}

/// Which sender to reach and the secret that proves we are its peer.
#[derive(Debug, Clone, Copy)]
pub enum Target<'a> {
    /// First contact: the ticket's address if it has one, else discovery
    /// by the code's nameplate; paired with the ticket's code.
    Pairing(&'a Ticket),
    /// Reconnect: discovery by resume tag, proven with the session's
    /// resume secret.
    Resume(&'a ResumeSecret),
}

impl Target<'_> {
    /// The secret the handshake proves.
    fn secret(self) -> Secret {
        match self {
            Self::Pairing(ticket) => Secret::Pair(ticket.code.clone()),
            Self::Resume(secret) => Secret::Resume(secret.clone()),
        }
    }

    /// The address the user typed, if any (otherwise: discovery).
    fn address(self) -> Option<SocketAddrV4> {
        match self {
            Self::Pairing(ticket) => ticket.addr,
            Self::Resume(_) => None,
        }
    }
}

/// Receiver side: find the sender of `target` and authenticate within
/// `timeout`.
///
/// Every error means no authenticated sender was reached (never an
/// authenticated peer's protocol error), so callers may retry. Discovered
/// candidates (a bounded number) race through the full handshake;
/// candidates that fail are dropped while the others continue. One call
/// sends only a few PAKE messages in total (each is a guess at the code
/// for whoever answers discovery). [`TransportError::PairingFailed`]
/// means every candidate that got as far as authentication failed it
/// (wrong code or secret, or impostors and no real sender within those
/// guesses). A sender whose handshake slots are busy is redialled until
/// `timeout`.
///
/// [`TransportError::PairingFailed`]: crate::TransportError::PairingFailed
pub fn connect(target: Target<'_>, timeout: Duration, opts: &ConnectOptions) -> Result<Connection> {
    let identity = Arc::new(Identity::generate()?);
    let (endpoint, config) = endpoint::client(opts.platform.as_ref(), &identity)?;
    let dialer = Dialer::new(
        endpoint.clone(),
        config,
        identity,
        target.secret(),
        Instant::now() + timeout,
    );
    let cancel = Watch::new(opts.cancel.as_ref());
    let auth = cancel.block_on(async {
        match target.address() {
            Some(addr) => dialer.authenticate(addr).await,
            None => race(dialer, &opts.platform).await,
        }
    })?;
    Ok(Connection::new(endpoint, auth, cancel))
}
