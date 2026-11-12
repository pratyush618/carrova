//! Every transport tunable in one place (sources and measurements:
//! `docs/research/phase3-performance.md`). Wire-protocol constants that
//! peers must agree on live in the crate root instead.

use std::time::Duration;

const MIB: usize = 1024 * 1024;

// ------------------------------------------------------------------ runtime

/// Tokio worker threads. Network I/O only (QUIC drivers, framing); disk
/// reads, BLAKE3 and engine callbacks never run on them.
pub(crate) const RUNTIME_WORKER_THREADS: usize = 2;

/// How often a blocking call checks the caller's cancel signal.
pub(crate) const CANCEL_POLL: Duration = Duration::from_millis(50);

// ------------------------------------------------------------------ socket

/// Requested `SO_SNDBUF` / `SO_RCVBUF` for the QUIC UDP socket (a
/// one-time warning with the platform's remedy is logged when the OS
/// grants less).
pub(crate) const UDP_SOCKET_BUFFER: usize = 8 * MIB;

// ------------------------------------------------------------------ QUIC

/// Per-stream flow-control window.
pub(crate) const STREAM_RECEIVE_WINDOW: u32 = 4 * MIB as u32;
/// Connection-wide flow-control window (bounds receiver memory).
pub(crate) const CONNECTION_RECEIVE_WINDOW: u32 = 32 * MIB as u32;
/// Bytes in flight across all streams.
pub(crate) const SEND_WINDOW: u64 = 32 * MIB as u64;
/// Bidirectional streams a peer may have open at once: control stream +
/// chunk streams + the fetch-done stream.
pub(crate) const MAX_CONCURRENT_BIDI_STREAMS: u32 = 16;
/// Initial RTT estimate (LAN; the default 333 ms slows early recovery).
pub(crate) const INITIAL_RTT: Duration = Duration::from_millis(10);
/// Connection idle timeout.
pub(crate) const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Keep-alive interval (keeps Wi-Fi pauses from killing the session).
pub(crate) const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(5);

/// TLS server name the receiver presents (certificates are self-signed and
/// authenticated by the PAKE, so the name only has to be well-formed).
pub(crate) const TLS_SERVER_NAME: &str = "carrova.invalid";

// ------------------------------------------------------------------ handshake

/// Longest one pairing / resume handshake may take once a connection is
/// up (bounded further by the caller's timeout).
pub(crate) const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Sender: longest an incoming connection may take to put the secret at
/// stake (QUIC/TLS handshake, `Hello`, and for pairing the receiver's PAKE
/// message). Keeps stalled or idle connections from holding a handshake
/// slot.
pub(crate) const PRE_PAKE_TIMEOUT: Duration = Duration::from_secs(2);
/// Sender: incoming handshakes run concurrently, at most this many at
/// once (further connections are refused until a slot frees up).
pub(crate) const MAX_PENDING_HANDSHAKES: usize = 4;
/// Sender: pending handshakes one source IP address may hold at once.
pub(crate) const MAX_PENDING_PER_ADDRESS: usize = 1;
/// Sender: resume-`Hello` nonces remembered (oldest forgotten first) so a
/// `Hello` cannot be replayed during the listener's lifetime.
pub(crate) const RESUME_NONCE_MEMORY: usize = 1024;
/// Receiver: code guesses (PAKE messages sent) one connect call may spend
/// across all candidates; more would let impostors answering discovery
/// collect guesses at the code.
pub(crate) const MAX_PAKE_GUESSES_PER_CONNECT: usize = 3;
/// Receiver: most candidate addresses one connect call dials (further
/// discovered addresses are ignored).
pub(crate) const MAX_RACED_CANDIDATES: usize = 8;
/// Receiver: wait before redialling a sender that refused the connection
/// because its handshake slots were busy.
pub(crate) const BUSY_RETRY: Duration = Duration::from_millis(250);
/// Receiver: once every discovered candidate failed authentication, how
/// long to wait for another candidate before reporting the pairing as
/// failed.
pub(crate) const RACE_SETTLE: Duration = Duration::from_secs(1);

/// Closing a connection: longest wait for the peer to acknowledge the rest
/// of the control stream before the close frame is sent.
pub(crate) const CLOSE_LINGER: Duration = Duration::from_secs(2);

// ------------------------------------------------------------------ chunks

/// Most parallel chunk streams one fetch may open: the stream limit minus
/// the control and fetch-done streams.
pub(crate) const MAX_PARALLEL_FETCHES: usize = MAX_CONCURRENT_BIDI_STREAMS as usize - 2;
/// Target bytes per chunk-request batch.
pub(crate) const REQUEST_BATCH_BYTES: u64 = MIB as u64;
/// Most hashes per chunk-request batch (keeps request frames small).
pub(crate) const REQUEST_BATCH_CHUNKS: usize = 1024;
/// Requested-but-unreceived bytes per chunk stream (≥ 2 batches in flight).
pub(crate) const STREAM_OUTSTANDING_BYTES: usize = 4 * MIB;
/// Verified-but-undelivered bytes between the network and the receiver's
/// chunk callback (backpressure towards the sender).
pub(crate) const DELIVERY_BUDGET_BYTES: usize = 16 * MIB;
/// Sender: chunk records are coalesced into writes of at most this size.
pub(crate) const RESPONSE_COALESCE_BYTES: usize = MIB;
/// Sender: read-but-unsent bytes per chunk stream.
pub(crate) const SERVE_STREAM_BUDGET_BYTES: usize = 8 * MIB;
/// Sender: queued disk reads across all streams.
pub(crate) const SERVE_READ_QUEUE: usize = 256;
/// Sender: one connection may request at most this many times the unique
/// bytes of the chunks it is allowed (re-requests after failed
/// verification included); more is a protocol violation.
pub(crate) const SERVED_BYTES_FACTOR: u64 = 3;

// ------------------------------------------------------------------ discovery

/// Re-send interval of the broadcast discovery query.
pub(crate) const BROADCAST_RETRY: Duration = Duration::from_millis(500);
/// Poll interval of the mDNS browse thread (how quickly it notices that
/// discovery ended).
pub(crate) const MDNS_POLL: Duration = Duration::from_millis(100);
/// Longest wait for the mDNS daemon to withdraw a replaced advertisement
/// (it acknowledges in milliseconds; the bound only guards a stuck daemon).
pub(crate) const MDNS_WITHDRAW_WAIT: Duration = Duration::from_secs(1);
/// Largest broadcast datagram read or sent.
pub(crate) const BROADCAST_MAX_DATAGRAM: usize = 512;
/// Lowest sender port a discovery answer may name: privileged ports are
/// never Carrova senders, so a spoofed answer cannot aim dials at them.
pub(crate) const MIN_SENDER_PORT: u16 = 1024;

/// Semaphore permits that account for `bytes` against a `budget`: one per
/// byte, capped at the budget so a single oversized item cannot wait
/// forever.
pub(crate) fn budget_permits(bytes: usize, budget: usize) -> u32 {
    u32::try_from(bytes.min(budget)).unwrap_or(u32::MAX)
}
