//! Sender side: bind, advertise, accept authenticated connections.

use std::net::SocketAddrV4;
use std::sync::Arc;
use std::time::Duration;

use carrova_crypto::pairing::{CertHash, PairingCode, ResumeSecret};
use carrova_platform::Platform;
use tokio::time::Instant;

use crate::accept;
use crate::cancel::{CancelSignal, Watch};
use crate::claim::CodeBook;
use crate::connection::Connection;
use crate::discovery::{Advertiser, Offer, Subject, primary_lan_ipv4};
use crate::endpoint;
use crate::error::{Result, TransportError};
use crate::handshake::Expected;
use crate::replay::ResumeNonces;
use crate::tls::Identity;
use crate::tuning::RESUME_NONCE_MEMORY;

/// How the sender listens.
#[derive(Debug, Clone)]
pub struct ListenOptions {
    /// UDP port for QUIC; `0` picks a free port.
    pub port: u16,
    /// Advertise over mDNS and answer broadcast queries. Off, only a
    /// [`crate::Ticket`] with an address reaches the sender.
    pub discovery: bool,
    /// Device name advertised while a code is offered, so receivers can
    /// list this sender ([`crate::browse`]); `None` advertises none. Pass
    /// a sanitised name: it is shown on other machines.
    pub device_name: Option<String>,
    /// OS access for sockets and interface listing (pass the engine's).
    pub platform: Arc<dyn Platform>,
    /// Stops accepting and every call on accepted connections
    /// ([`crate::TransportError::Cancelled`]); `None` = not cancellable.
    pub cancel: Option<Arc<dyn CancelSignal>>,
}

impl Default for ListenOptions {
    /// Any port, discovery on, the platform this binary was built for.
    fn default() -> Self {
        Self {
            port: 0,
            discovery: true,
            device_name: None,
            platform: Arc::from(carrova_platform::current()),
            cancel: None,
        }
    }
}

/// A bound QUIC endpoint on the sender plus its discovery advertisement.
/// Advertisements stop when the listener is dropped; accepted connections
/// stay open.
#[derive(Debug)]
pub struct Listener {
    endpoint: quinn::Endpoint,
    identity: Arc<Identity>,
    platform: Arc<dyn Platform>,
    port: u16,
    advertiser: Option<Advertiser>,
    /// Every code offered so far, each single-use.
    codes: CodeBook,
    /// Resume-`Hello` nonces already admitted.
    nonces: Arc<ResumeNonces>,
    /// The caller's cancel signal.
    cancel: Watch,
}

impl Listener {
    /// Bind the QUIC endpoint (IPv4) and start discovery responders.
    pub fn bind(opts: ListenOptions) -> Result<Self> {
        let identity = Arc::new(Identity::generate()?);
        let endpoint = endpoint::server(opts.platform.as_ref(), &identity, opts.port)?;
        let port = endpoint::local_v4(&endpoint)?.port();
        let advertiser = if opts.discovery {
            // Unique per listener without extra randomness: the throwaway
            // certificate's hash.
            let hash = CertHash::of(identity.cert_der()).to_string();
            Some(Advertiser::start(
                opts.platform.as_ref(),
                port,
                format!("carrova-{}", &hash[..12]),
                opts.device_name,
            )?)
        } else {
            None
        };
        Ok(Self {
            endpoint,
            identity,
            platform: opts.platform,
            port,
            advertiser,
            codes: CodeBook::default(),
            nonces: Arc::new(ResumeNonces::new(RESUME_NONCE_MEMORY)),
            cancel: Watch::new(opts.cancel.as_ref()),
        })
    }

    /// Advertise `code`'s nameplate (never the words) for pairing. Replaces
    /// any previous advertisement.
    pub fn advertise_pairing(&mut self, code: &PairingCode) -> Result<()> {
        self.advertise(Offer::new(Subject::Pair(code.nameplate()))?);
        Ok(())
    }

    /// Advertise the resumable session of `secret` under a fresh resume
    /// tag (new random nonce on every call, so adverts are unlinkable).
    /// Replaces any previous advertisement.
    pub fn advertise_resume(&mut self, secret: &ResumeSecret) -> Result<()> {
        self.advertise(Offer::new(Subject::Resume(secret.clone()))?);
        Ok(())
    }

    fn advertise(&mut self, offer: Offer) {
        if let Some(a) = &mut self.advertiser {
            a.set(offer);
        }
    }

    /// Wait for a receiver and pair with `code`. Any failed attempt returns
    /// [`TransportError::PairingFailed`]: the caller must burn the code and
    /// advertise a fresh one (one guess per code). The listener enforces
    /// this: a code is single-use, so a code that failed (or already
    /// paired) fails immediately.
    ///
    /// Receivers asking for another nameplate are turned away without
    /// touching the code. Handshakes run concurrently; the first receiver
    /// whose PAKE message arrives takes the code. If `timeout` passes while
    /// that receiver's handshake runs, the call waits for it (bounded), so
    /// a real receiver is never cut off and reported as a wrong code.
    ///
    /// `on_pake` runs once, on the caller's thread, when a receiver's PAKE
    /// message claims the code (the code is being checked); success or
    /// [`TransportError::PairingFailed`] follows.
    pub fn accept_pairing(
        &mut self,
        code: &PairingCode,
        timeout: Duration,
        mut on_pake: impl FnMut(),
    ) -> Result<Connection> {
        let claim = self.codes.claim(code);
        if claim.is_taken() {
            return Err(TransportError::PairingFailed);
        }
        self.accept(
            &Expected::Pair {
                code: code.clone(),
                claim,
            },
            timeout,
            &mut on_pake,
        )
    }

    /// Wait for the session's receiver to reconnect, proven with `secret`.
    pub fn accept_resume(
        &mut self,
        secret: &ResumeSecret,
        timeout: Duration,
    ) -> Result<Connection> {
        self.accept(
            &Expected::Resume {
                secret: secret.clone(),
                nonces: Arc::clone(&self.nonces),
            },
            timeout,
            &mut || {},
        )
    }

    /// Handshakes still running when the caller cancels are dropped.
    fn accept(
        &self,
        expected: &Expected,
        timeout: Duration,
        on_staked: &mut dyn FnMut(),
    ) -> Result<Connection> {
        let deadline = Instant::now() + timeout;
        let auth = self.cancel.block_on(accept::accept(
            &self.endpoint,
            &self.identity,
            expected,
            deadline,
            on_staked,
        ))?;
        Ok(Connection::new(
            self.endpoint.clone(),
            auth,
            self.cancel.clone(),
        ))
    }

    /// UDP port of the QUIC endpoint (all IPv4 interfaces).
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// `<ipv4>:<port>` for a fallback ticket, if a LAN address is known.
    #[must_use]
    pub fn fallback_addr(&self) -> Option<SocketAddrV4> {
        primary_lan_ipv4(self.platform.as_ref()).map(|ip| SocketAddrV4::new(ip, self.port))
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        // Refuse new connections; accepted ones stay open.
        self.endpoint.set_server_config(None);
    }
}
