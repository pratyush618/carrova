use std::net::Ipv4Addr;

use super::*;

#[test]
fn guess_budget_is_spent_exactly_once_per_guess() {
    let budget = PakeBudget::new(2);
    assert!(budget.spend());
    assert!(!budget.exhausted());
    assert!(budget.spend());
    assert!(budget.exhausted());
    assert!(!budget.spend());
}

#[test]
fn candidates_are_deduplicated_and_capped() {
    let mut candidates = Candidates::default();
    let addr = |port| SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), port);
    assert!(candidates.admit(addr(2000)));
    assert!(!candidates.admit(addr(2000)));
    let admitted = (2001..2100).filter(|&p| candidates.admit(addr(p))).count();
    assert_eq!(admitted + 1, MAX_RACED_CANDIDATES);
    assert!(candidates.is_full());
}

// ------------------------------------------------------------ one sender

mod one_sender {
    use std::net::{Ipv4Addr, SocketAddrV4};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use carrova_crypto::pairing::{PairingCode, ResumeSecret};
    use tokio::sync::mpsc;
    use tokio::time::Instant;

    use super::super::{Dialer, race_found};
    use crate::endpoint;
    use crate::error::TransportError;
    use crate::handshake::Secret;
    use crate::runtime::runtime;
    use crate::tls::Identity;
    use crate::wire::{Handshake, StreamReader};

    /// A sender that counts the `Hello`s it receives and never answers,
    /// so every candidate the receiver lets through shows up in the count.
    struct SilentSender {
        _endpoint: quinn::Endpoint,
        port: u16,
        hellos: Arc<AtomicUsize>,
    }

    impl SilentSender {
        fn start() -> Self {
            let platform = carrova_platform::current();
            let identity = Identity::generate().unwrap();
            let ep = endpoint::server(platform.as_ref(), &identity, 0).unwrap();
            let port = endpoint::local_v4(&ep).unwrap().port();
            let hellos = Arc::new(AtomicUsize::new(0));
            let (accepting, counter) = (ep.clone(), Arc::clone(&hellos));
            runtime().unwrap().spawn(async move {
                while let Some(incoming) = accepting.accept().await {
                    tokio::spawn(count_hello(incoming, Arc::clone(&counter)));
                }
            });
            Self {
                _endpoint: ep,
                port,
                hellos,
            }
        }

        /// The sender's port on `127.0.0.<n>`: one sender, many addresses.
        fn at(&self, n: u8) -> SocketAddrV4 {
            SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, n), self.port)
        }
    }

    async fn count_hello(incoming: quinn::Incoming, hellos: Arc<AtomicUsize>) {
        let Ok(conn) = incoming.await else { return };
        let Ok((_send, recv)) = conn.accept_bi().await else {
            return;
        };
        if let Ok(Handshake::Hello { .. }) = StreamReader::new(recv).expect_frame().await {
            hellos.fetch_add(1, Ordering::SeqCst);
        }
        conn.closed().await;
    }

    /// Race `secret` over `candidates`; the `Hello`s `senders` received.
    fn hellos_from_a_race(
        secret: Secret,
        senders: &[SilentSender],
        candidates: &[SocketAddrV4],
    ) -> usize {
        let platform = carrova_platform::current();
        let identity = Arc::new(Identity::generate().unwrap());
        let (ep, config) = endpoint::client(platform.as_ref(), &identity).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let dialer = Dialer::new(ep, config, identity, secret, deadline);
        let (found, rx) = mpsc::channel(candidates.len());
        for &addr in candidates {
            found.try_send(addr).unwrap();
        }
        drop(found);
        let result = runtime().unwrap().block_on(race_found(dialer, rx));
        assert!(matches!(result, Err(TransportError::Timeout)), "{result:?}");
        senders
            .iter()
            .map(|s| s.hellos.load(Ordering::SeqCst))
            .sum()
    }

    /// Hellos one sender discovered at two addresses receives.
    fn hellos_from_a_dual_address_race(secret: Secret) -> usize {
        let sender = SilentSender::start();
        let candidates = [sender.at(1), sender.at(2)];
        hellos_from_a_race(secret, &[sender], &candidates)
    }

    #[test]
    fn resume_runs_one_handshake_per_sender_however_many_addresses_it_has() {
        let secret = Secret::Resume(ResumeSecret::from_bytes([7; 32]));
        assert_eq!(hellos_from_a_dual_address_race(secret), 1);
    }

    #[test]
    fn pairing_runs_one_handshake_per_sender_however_many_addresses_it_has() {
        let secret = Secret::Pair(PairingCode::generate().unwrap());
        assert_eq!(hellos_from_a_dual_address_race(secret), 1);
    }

    #[test]
    fn distinct_senders_each_get_a_handshake() {
        let senders = [SilentSender::start(), SilentSender::start()];
        let candidates = [senders[0].at(1), senders[1].at(1)];
        let secret = Secret::Resume(ResumeSecret::from_bytes([7; 32]));
        assert_eq!(hellos_from_a_race(secret, &senders, &candidates), 2);
    }
}

// ------------------------------------------------------------ a real sender

mod real_sender {
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use carrova_crypto::pairing::{CertHash, PairingCode, ResumeSecret};
    use serde::{Deserialize, Serialize};
    use tokio::sync::mpsc;
    use tokio::time::Instant;

    use super::super::{Dialer, SenderClaims, race_found};
    use crate::cancel::Watch;
    use crate::connection::Connection;
    use crate::endpoint;
    use crate::error::Result;
    use crate::handshake::{Authenticated, Secret};
    use crate::runtime::runtime;
    use crate::tls::{self, Identity};
    use crate::wire::CloseCode;
    use crate::{ListenOptions, Listener};

    const T: Duration = Duration::from_secs(10);

    /// What the sender accepts; the receiver connects with the matching
    /// secret. No discovery: the receiver dials explicit addresses, so
    /// parallel tests never reach each other's senders.
    #[derive(Clone)]
    enum Offer {
        Pair(PairingCode),
        Resume(ResumeSecret),
    }

    impl Offer {
        fn pairing() -> Self {
            Self::Pair(PairingCode::generate().unwrap())
        }

        fn resume() -> Self {
            Self::Resume(ResumeSecret::from_bytes([9; 32]))
        }

        fn secret(&self) -> Secret {
            match self {
                Self::Pair(code) => Secret::Pair(code.clone()),
                Self::Resume(secret) => Secret::Resume(secret.clone()),
            }
        }

        fn accept(&self, listener: &mut Listener) -> Result<Connection> {
            match self {
                Self::Pair(code) => listener.accept_pairing(code, T, || {}),
                Self::Resume(secret) => listener.accept_resume(secret, T),
            }
        }
    }

    /// A real listener (bound to `0.0.0.0`) accepting `offer` on a thread:
    /// its port and the accept call's outcome.
    fn accepting(offer: &Offer) -> (u16, thread::JoinHandle<Result<Connection>>) {
        let mut listener = Listener::bind(ListenOptions {
            discovery: false,
            ..ListenOptions::default()
        })
        .unwrap();
        let port = listener.port();
        let offer = offer.clone();
        (port, thread::spawn(move || offer.accept(&mut listener)))
    }

    fn at(port: u16, n: u8) -> SocketAddrV4 {
        SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, n), port)
    }

    /// One connect call's dialer with `budget` until its deadline.
    fn dialer(secret: Secret, budget: Duration) -> (Dialer, Arc<Identity>) {
        let platform = carrova_platform::current();
        let identity = Arc::new(Identity::generate().unwrap());
        let (ep, config) = endpoint::client(platform.as_ref(), &identity).unwrap();
        let deadline = Instant::now() + budget;
        let dialer = Dialer::new(ep, config, Arc::clone(&identity), secret, deadline);
        (dialer, identity)
    }

    fn race(dialer: Dialer, candidates: &[SocketAddrV4]) -> Result<Authenticated> {
        let (found, rx) = mpsc::channel(candidates.len());
        for &addr in candidates {
            found.try_send(addr).unwrap();
        }
        drop(found);
        runtime().unwrap().block_on(race_found(dialer, rx))
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Ping(u32);

    /// Both sides succeeded on one and the same connection: the sender
    /// authenticated this receiver, and a message crosses between them.
    fn assert_one_session(
        sender: Result<Connection>,
        receiver: Result<Authenticated>,
        endpoint: quinn::Endpoint,
        identity: &Identity,
    ) {
        let mut sender = sender.expect("the sender must not fail over a duplicate");
        let receiver = receiver.expect("the receiver must authenticate");
        let mut receiver = Connection::new(endpoint, receiver, Watch::default());
        assert_eq!(sender.peer_cert_hash(), CertHash::of(identity.cert_der()));
        assert_eq!(
            sender.resume_secret().expose_bytes(),
            receiver.resume_secret().expose_bytes()
        );
        sender.send(&Ping(7)).unwrap();
        assert_eq!(receiver.recv::<Ping>(T).unwrap(), Ping(7));
    }

    /// The receiver races a real sender at two of its addresses. Whatever
    /// the interleaving, both sides succeed on one connection. (Both dials
    /// leave from `127.0.0.1`, so the sender's per-address admission may
    /// refuse the second until the first is done; the fixed-order test
    /// below drives the claim path itself.)
    fn race_one_sender_at_two_addresses(offer: &Offer) {
        let (port, sender) = accepting(offer);
        let (dialer, identity) = dialer(offer.secret(), T);
        let endpoint = dialer.endpoint.clone();
        let receiver = race(dialer, &[at(port, 1), at(port, 2)]);
        assert_one_session(sender.join().unwrap(), receiver, endpoint, &identity);
    }

    #[test]
    fn pairing_race_at_two_addresses_succeeds_on_both_sides() {
        race_one_sender_at_two_addresses(&Offer::pairing());
    }

    #[test]
    fn resume_race_at_two_addresses_succeeds_on_both_sides() {
        race_one_sender_at_two_addresses(&Offer::resume());
    }

    /// The race's steps in a fixed order: the first connection claims the
    /// sender, the duplicate at its other address reaches TLS and is
    /// closed before `Hello`, then the first authenticates. The sender
    /// must shrug the duplicate off (not fail, not burn its code).
    ///
    /// The duplicate leaves from `127.0.0.2`, as a dial to a LAN address
    /// leaves from the receiver's LAN address: from the same source the
    /// sender would refuse it while the first holds that address's
    /// handshake slot.
    fn duplicate_closed_before_hello(offer: &Offer) {
        let (port, sender) = accepting(offer);
        let (dialer, identity) = dialer(offer.secret(), T);
        let endpoint = dialer.endpoint.clone();
        let claims = Arc::new(SenderClaims::default());
        let receiver = runtime().unwrap().block_on(async {
            let first = dialer.dial(at(port, 1)).await.unwrap();
            assert!(claims.claim(CertHash::of(&tls::peer_cert(&first).unwrap())));
            let elsewhere = quinn::Endpoint::client(SocketAddr::V4(at(0, 2))).unwrap();
            let duplicate = Dialer {
                endpoint: elsewhere,
                ..dialer.clone()
            }
            .authenticate_once(at(port, 2), Arc::clone(&claims))
            .await;
            assert!(matches!(duplicate, Ok(None)), "{duplicate:?}");
            dialer.handshake(first).await
        });
        assert_one_session(sender.join().unwrap(), receiver, endpoint, &identity);
    }

    #[test]
    fn pairing_sender_shrugs_off_a_duplicate_closed_before_hello() {
        duplicate_closed_before_hello(&Offer::pairing());
    }

    #[test]
    fn resume_sender_shrugs_off_a_duplicate_closed_before_hello() {
        duplicate_closed_before_hello(&Offer::resume());
    }

    /// A sender that closes every connection as soon as TLS completes.
    fn closing_sender() -> (quinn::Endpoint, u16) {
        let platform = carrova_platform::current();
        let identity = Identity::generate().unwrap();
        let ep = endpoint::server(platform.as_ref(), &identity, 0).unwrap();
        let port = endpoint::local_v4(&ep).unwrap().port();
        let accepting = ep.clone();
        runtime().unwrap().spawn(async move {
            while let Some(incoming) = accepting.accept().await {
                tokio::spawn(async move {
                    if let Ok(conn) = incoming.await {
                        CloseCode::Normal.close(&conn);
                    }
                });
            }
        });
        (ep, port)
    }

    /// Known limit: the claimed connection dying before auth is not a
    /// wrong code, and the duplicate is not retried; the call ends at its
    /// deadline with a retryable error.
    #[test]
    fn claimed_connection_lost_before_auth_is_retryable_not_a_pairing_failure() {
        let (_sender, port) = closing_sender();
        let secret = Secret::Pair(PairingCode::generate().unwrap());
        let (dialer, _) = dialer(secret, Duration::from_secs(2));
        let result = race(dialer, &[at(port, 1), at(port, 2)]);
        // `is_lost`: `PeerLost` or `Timeout`, never `PairingFailed`.
        assert!(matches!(&result, Err(e) if e.is_lost()), "{result:?}");
    }
}
