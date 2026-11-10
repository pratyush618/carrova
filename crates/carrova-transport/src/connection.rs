//! An authenticated connection: control messages and chunk transfer.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use carrova_crypto::pairing::{CertHash, ResumeSecret};
use carrova_manifest::{ChunkHash, ChunkRef, ChunkSource, Manifest};
use quinn::SendStream;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::time::timeout;

use crate::cancel::Watch;
use crate::error::{Result, TransportError};
use crate::handshake::Authenticated;
use crate::runtime::runtime;
use crate::tuning::CLOSE_LINGER;
use crate::wire::{CloseCode, StreamReader, encode_frame};
use crate::{fetch, serve};

/// Receiver callback for each verified chunk ([`Connection::fetch_chunks`]).
pub type OnChunk<'a> = dyn FnMut(&ChunkRef, Vec<u8>) -> Result<(), FetchAbort> + 'a;

/// A mutually authenticated connection. Exists only after pairing (or the
/// resume proof) succeeded on both sides. Dropping it closes the
/// connection. Its blocking calls (except [`Self::send`]) watch the cancel
/// signal of the options it was made with.
#[derive(Debug)]
pub struct Connection {
    /// Keeps the receiver's own socket alive (shared with the listener on
    /// the sender).
    _endpoint: quinn::Endpoint,
    conn: quinn::Connection,
    control_send: SendStream,
    control_recv: StreamReader,
    peer_cert: CertHash,
    resume: ResumeSecret,
    /// Chunk bytes the peer requested from us so far (L1 ceiling).
    requested: Arc<AtomicU64>,
    /// The caller's cancel signal.
    cancel: Watch,
    /// How dropping the connection closes it.
    close: CloseCode,
}

impl Connection {
    /// The only constructor: requires a completed handshake.
    pub(crate) fn new(endpoint: quinn::Endpoint, auth: Authenticated, cancel: Watch) -> Self {
        Self {
            _endpoint: endpoint,
            conn: auth.conn,
            control_send: auth.send,
            control_recv: auth.recv,
            peer_cert: auth.peer_cert,
            resume: auth.resume,
            requested: Arc::new(AtomicU64::new(0)),
            cancel,
            close: CloseCode::Normal,
        }
    }

    /// Peer's socket address.
    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.conn.remote_address()
    }

    /// SHA-256 of the peer's certificate on this connection.
    #[must_use]
    pub fn peer_cert_hash(&self) -> CertHash {
        self.peer_cert
    }

    /// The session's resume secret (from pairing, or the one used to resume).
    #[must_use]
    pub fn resume_secret(&self) -> &ResumeSecret {
        &self.resume
    }

    /// Send one control message (CBOR frame on the control stream). Not
    /// cancellable: control messages are small, and a farewell (e.g. an
    /// abort before [`Self::close_cancelled`]) must still go out after a
    /// cancel.
    pub fn send<T: Serialize>(&mut self, msg: &T) -> Result<()> {
        let frame = encode_frame(msg)?;
        runtime()?
            .block_on(self.control_send.write_all(&frame))
            .map_err(|e| TransportError::write(&e))
    }

    /// Receive the next control message, waiting at most `timeout`. A
    /// timeout or cancel loses nothing: a partly received message stays
    /// buffered.
    ///
    /// The peer ending the control stream means it is closing: the
    /// error says why ([`TransportError::PeerCancelled`],
    /// [`TransportError::PeerLost`], …).
    pub fn recv<T: DeserializeOwned>(&mut self, timeout_after: Duration) -> Result<T> {
        let Self {
            conn,
            control_recv,
            cancel,
            ..
        } = self;
        // The timer must be created inside the runtime.
        cancel.block_on(async {
            let frame = timeout(timeout_after, control_recv.frame())
                .await
                .map_err(|_| TransportError::Timeout)??;
            match frame {
                Some(msg) => Ok(msg),
                None => Err(TransportError::closed_by_peer(conn).await),
            }
        })
    }

    /// Receiver: pull `chunks` (in the given priority order) over up to
    /// `parallel` streams. Each chunk is length- and BLAKE3-verified before
    /// `on_chunk` sees it; a bad chunk is re-requested up to
    /// [`crate::MAX_CHUNK_RETRIES`] times. `on_chunk` runs on the calling
    /// thread, one chunk at a time; returning [`FetchAbort`] stops the
    /// fetch with [`TransportError::Aborted`]. Chunks the sender cannot
    /// supply or that keep failing verification are listed in the report;
    /// the rest of the fetch continues. Duplicate hashes are fetched once.
    ///
    /// If the connection drops, returns [`TransportError::PeerLost`]
    /// ([`TransportError::PeerCancelled`] if the sender cancelled); every
    /// chunk already passed to `on_chunk` stays delivered. A cancel ends
    /// the fetch phase with the sender cleanly, then returns
    /// [`TransportError::Cancelled`]; the connection stays usable.
    pub fn fetch_chunks(
        &mut self,
        chunks: &[ChunkRef],
        parallel: usize,
        on_chunk: &mut OnChunk<'_>,
    ) -> Result<FetchReport> {
        fetch::fetch(&self.conn, chunks, parallel, &self.cancel, on_chunk)
    }

    /// Sender: answer chunk requests from `source`, refusing any chunk not
    /// in `allowed`, until the receiver ends the fetch phase (or closes
    /// the connection normally). A request outside `allowed` closes the
    /// connection with [`TransportError::Violation`], as does a receiver
    /// requesting, over the life of this connection, more than a small
    /// multiple of the allowed chunks' unique bytes (re-requests included).
    /// `source` is read on the calling thread. A cancel resets the chunk
    /// streams and returns [`TransportError::Cancelled`]; follow it with
    /// [`Self::close_cancelled`] so the receiver learns why.
    pub fn serve_chunks(
        &mut self,
        allowed: &AllowedChunks,
        source: &mut dyn ChunkSource,
    ) -> Result<ServeEnd> {
        serve::serve(&self.conn, allowed, source, &self.requested, &self.cancel)
    }

    /// Close the connection gracefully (best effort): control messages
    /// already sent are delivered first (waiting at most a short linger),
    /// then the peer is told. Dropping the connection does the same.
    pub fn close(self) {}

    /// Like [`Self::close`], telling the peer this side cancelled the
    /// session: its pending and later calls on the connection end with
    /// [`TransportError::PeerCancelled`].
    pub fn close_cancelled(mut self) {
        self.close = CloseCode::Cancelled;
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // quinn abandons unacknowledged stream data on close (and does not
        // even send the close frame while data is pending), so finish the
        // control stream and let the peer acknowledge it first.
        if self.control_send.finish().is_ok()
            && let Ok(rt) = runtime()
        {
            let stopped = self.control_send.stopped();
            let _ = rt.block_on(async { timeout(CLOSE_LINGER, stopped).await });
        }
        self.close.close(&self.conn);
    }
}

/// Chunks a sender is willing to serve: exactly those its manifest names.
/// Cheap to clone (shared).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllowedChunks(Arc<Allowed>);

#[derive(Debug, Default, PartialEq, Eq)]
struct Allowed {
    chunks: HashMap<ChunkHash, ChunkRef>,
    /// Sum of the distinct chunks' lengths.
    unique_bytes: u64,
}

impl AllowedChunks {
    /// Every unique chunk of `manifest`.
    #[must_use]
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let chunks: HashMap<ChunkHash, ChunkRef> =
            manifest.unique_chunk_refs().map(|c| (c.hash, *c)).collect();
        let unique_bytes = chunks.values().map(|c| u64::from(c.len)).sum();
        Self(Arc::new(Allowed {
            chunks,
            unique_bytes,
        }))
    }

    /// The manifest's reference for `hash`, if allowed.
    #[must_use]
    pub fn get(&self, hash: &ChunkHash) -> Option<&ChunkRef> {
        self.0.chunks.get(hash)
    }

    /// Number of distinct chunks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.chunks.len()
    }

    /// No chunks allowed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.chunks.is_empty()
    }

    /// Total length of the distinct chunks.
    #[must_use]
    pub fn unique_bytes(&self) -> u64 {
        self.0.unique_bytes
    }
}

/// Why the receiver's chunk callback stopped a fetch.
#[derive(Debug, thiserror::Error)]
pub enum FetchAbort {
    /// The receiver ran out of disk space; the fetch can resume later.
    #[error("receiver is out of disk space")]
    OutOfSpace,
    /// Storing the chunk failed for another reason.
    #[error("receiver could not store a chunk: {0}")]
    Failed(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Why one chunk could not be fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkFailure {
    /// The sender could not supply it (file changed or vanished since
    /// capture).
    Unavailable,
    /// Received bytes failed verification on every attempt.
    VerificationFailed,
}

/// One chunk that was not fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailedChunk {
    /// The chunk requested.
    pub chunk: ChunkRef,
    /// Why it failed.
    pub reason: ChunkFailure,
}

/// Outcome of [`Connection::fetch_chunks`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchReport {
    /// Chunks delivered to the callback.
    pub chunks: u64,
    /// Bytes delivered to the callback.
    pub bytes: u64,
    /// Chunks that could not be fetched (the caller fails their items).
    pub failed: Vec<FailedChunk>,
}

/// Outcome of [`Connection::serve_chunks`]: the receiver ended the fetch
/// phase.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ServeEnd {
    /// Chunks sent.
    pub chunks: u64,
    /// Chunk bytes sent.
    pub bytes: u64,
}
