//! File offer and transfer on a `File` lane.
//!
//! Wire (opener = sender), all frames length + CBOR:
//! 1. sender → `FileOffer`; receiver → `OfferAnswer`.
//! 2. On `Accept { resume_from }`: per chunk, `FileFrame::Chunk { len, hash }`
//!    (len ≤ 1 MiB, BLAKE3 of the chunk) then `len` raw bytes, until `size`.
//! 3. sender → `FileFrame::Done`; receiver commits, then → `()` and finishes.
//!
//! A dropped sender lane looks finished to the receiver, so only `Done` after
//! exactly `size` verified bytes commits; anything else aborts the sink.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

use carrova_transport::{Connection, Stream};
use serde::{Deserialize, Serialize};

use crate::LinkError;
use crate::lane::{LaneKind, open_lane};
use crate::name::safe_file_name;

/// Largest chunk on the wire.
const CHUNK: usize = 1 << 20;
/// How long the sender waits for an answer: the receiver's 60 s ask plus margin.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(75);
/// How long either side waits for the next frame or chunk mid-transfer.
const FRAME_TIMEOUT: Duration = Duration::from_secs(30);

/// A file one device offers another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOffer {
    /// Sender-chosen offer id.
    pub id: u64,
    /// Name as the sender knows it; [`receive_file`] normalises it with
    /// [`safe_file_name`] before anyone on the receiving side sees it.
    pub name: String,
    /// Size in bytes.
    pub size: u64,
    /// Media type, if known.
    pub mime: Option<String>,
}

/// The receiver's reply to a [`FileOffer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferAnswer {
    /// Send the file, starting at this byte offset.
    Accept {
        /// First byte the receiver still needs.
        resume_from: u64,
    },
    /// The user said no.
    Decline,
    /// The offer timed out unanswered.
    Expired,
}

/// Where received bytes go. Implementations write to a temporary place and
/// only expose the file under its final name on [`FileSink::commit`].
pub trait FileSink: Send {
    /// Write `bytes` at `offset`.
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()>;
    /// Every byte arrived and was verified: publish the file, return where.
    fn commit(self: Box<Self>) -> io::Result<PathBuf>;
    /// The transfer failed or was refused: discard anything written.
    fn abort(self: Box<Self>);
}

/// After the answer, what the sender sends next.
#[derive(Serialize, Deserialize)]
enum FileFrame {
    /// `len` raw bytes with this BLAKE3 hash follow.
    Chunk { len: u32, hash: [u8; 32] },
    /// All `size` bytes were sent.
    Done,
}

/// Offer `offer` on a new `File` lane and, if accepted, send `src` from the
/// answer's `resume_from`. `on_progress` gets the bytes sent so far. Succeeds
/// only once the receiver confirms it saved the file.
pub fn offer_file(
    conn: &Connection,
    offer: &FileOffer,
    src: &mut (dyn ReadSeek + '_),
    on_progress: &mut dyn FnMut(u64),
) -> Result<(), LinkError> {
    let mut stream = open_lane(conn, LaneKind::File)?;
    stream.send(offer)?;
    let mut offset = match stream.recv(ANSWER_TIMEOUT)? {
        OfferAnswer::Accept { resume_from } if resume_from <= offer.size => resume_from,
        OfferAnswer::Accept { .. } => return Err(LinkError::Protocol("resume past end")),
        OfferAnswer::Decline => return Err(LinkError::Declined),
        OfferAnswer::Expired => return Err(LinkError::Expired),
    };
    src.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0; CHUNK];
    while offset < offer.size {
        let chunk = &mut buf[..chunk_len(offer.size - offset)];
        src.read_exact(chunk)?;
        stream.send(&FileFrame::Chunk {
            len: chunk_u32(chunk.len()),
            hash: *blake3::hash(chunk).as_bytes(),
        })?;
        stream.write_all(chunk)?;
        offset += chunk.len() as u64;
        on_progress(offset);
    }
    stream.send(&FileFrame::Done)?;
    stream.recv::<()>(FRAME_TIMEOUT)?;
    Ok(())
}

/// Answer the offer on an accepted `File` lane and receive the file into
/// `sink`. `decide` sees the offer with its name already normalised by
/// [`safe_file_name`]. Commits only after `Done` and exactly `size` verified
/// bytes; on decline, expiry or any failure the sink is aborted.
/// `on_progress` gets the bytes held so far.
pub fn receive_file(
    mut stream: Stream,
    decide: &mut dyn FnMut(&FileOffer) -> OfferAnswer,
    mut sink: Box<dyn FileSink>,
    on_progress: &mut dyn FnMut(u64),
) -> Result<PathBuf, LinkError> {
    let received = answer(&mut stream, decide).and_then(|(from, size)| {
        receive_bytes(&mut stream, from, size, sink.as_mut(), on_progress)
    });
    if let Err(e) = received {
        sink.abort();
        return Err(e);
    }
    let path = sink.commit()?;
    stream.send(&())?;
    stream.finish()?;
    Ok(path)
}

/// Readable, seekable source of an offered file.
pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}

/// Read the offer, ask `decide` with the safe name, send the answer.
/// Accepted → `(resume_from, size)`.
fn answer(
    stream: &mut Stream,
    decide: &mut dyn FnMut(&FileOffer) -> OfferAnswer,
) -> Result<(u64, u64), LinkError> {
    let mut offer: FileOffer = stream.recv(FRAME_TIMEOUT)?;
    offer.name = safe_file_name(&offer.name);
    let answer = decide(&offer);
    let from = match answer {
        OfferAnswer::Accept { resume_from } if resume_from <= offer.size => resume_from,
        OfferAnswer::Accept { .. } => return Err(LinkError::Protocol("resume past end")),
        OfferAnswer::Decline | OfferAnswer::Expired => {
            stream.send(&answer)?;
            return Err(if answer == OfferAnswer::Decline {
                LinkError::Declined
            } else {
                LinkError::Expired
            });
        }
    };
    stream.send(&answer)?;
    Ok((from, offer.size))
}

/// Receive verified chunks covering `from..size`, then the `Done` frame.
fn receive_bytes(
    stream: &mut Stream,
    mut offset: u64,
    size: u64,
    sink: &mut dyn FileSink,
    on_progress: &mut dyn FnMut(u64),
) -> Result<(), LinkError> {
    let mut buf = vec![0; CHUNK];
    loop {
        match stream.recv(FRAME_TIMEOUT)? {
            FileFrame::Done if offset == size => return Ok(()),
            FileFrame::Done => return Err(LinkError::Protocol("file ended early")),
            FileFrame::Chunk { len, hash } => {
                let len = usize::try_from(len).unwrap_or(usize::MAX);
                if len == 0 || len > chunk_len(size - offset) {
                    return Err(LinkError::Protocol("chunk outside the offered size"));
                }
                let chunk = &mut buf[..len];
                stream.read_exact(chunk, FRAME_TIMEOUT)?;
                if blake3::hash(chunk).as_bytes() != &hash {
                    return Err(LinkError::Protocol("chunk hash mismatch"));
                }
                sink.write_at(offset, chunk)?;
                offset += len as u64;
                on_progress(offset);
            }
        }
    }
}

/// Bytes in the next chunk when `left` remain: at most [`CHUNK`].
fn chunk_len(left: u64) -> usize {
    usize::try_from(left).map_or(CHUNK, |left| left.min(CHUNK))
}

/// A chunk length as sent on the wire; chunks never exceed [`CHUNK`].
fn chunk_u32(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}
