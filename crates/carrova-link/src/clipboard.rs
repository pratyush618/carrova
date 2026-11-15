//! Clipboard items, their size limits, the `Clipboard` lane and the receive-side history.
//!
//! Wire (opener = sender), frames are length + CBOR:
//! 1. sender → `ClipHeader { kind, len }`, then exactly `len` raw bytes.
//! 2. receiver checks `len` against the cap *before* reading any payload, then
//!    validates the bytes (UTF-8 text, PNG signature) and replies `()`.

use std::collections::VecDeque;
use std::fmt;
use std::time::Duration;

use carrova_transport::{Connection, Stream};
use serde::{Deserialize, Serialize};

use crate::LinkError;
use crate::lane::{LaneKind, open_lane};

/// Largest text clip accepted, in bytes.
pub const MAX_CLIP_TEXT: usize = 1 << 20;
/// Largest PNG clip accepted, in bytes.
pub const MAX_CLIP_PNG: usize = 10 << 20;
/// Clips kept in [`ClipHistory`].
const HISTORY_CAP: usize = 20;
/// Payload bytes read or written per call, so a stalled peer is noticed between slices.
const SLICE: usize = 1 << 20;
/// How long either side waits for the next frame or slice.
const CLIP_TIMEOUT: Duration = Duration::from_secs(30);
/// First eight bytes of every PNG file.
const PNG_SIGNATURE: [u8; 8] = *b"\x89PNG\r\n\x1a\n";

/// What the payload bytes are.
#[derive(Serialize, Deserialize, Clone, Copy)]
enum ClipKind {
    Text,
    Png,
}

impl ClipKind {
    fn max(self) -> usize {
        match self {
            Self::Text => MAX_CLIP_TEXT,
            Self::Png => MAX_CLIP_PNG,
        }
    }
}

/// First frame on a `Clipboard` lane.
#[derive(Serialize, Deserialize)]
struct ClipHeader {
    kind: ClipKind,
    len: u64,
}

/// One clipboard payload. `Debug` prints only the kind and size, never contents.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipItem {
    /// UTF-8 text.
    Text(String),
    /// PNG-encoded image.
    Png(Vec<u8>),
}

impl ClipItem {
    /// Refuses items over their size limit.
    pub fn check(&self) -> Result<(), LinkError> {
        let (len, max) = match self {
            Self::Text(t) => (t.len(), MAX_CLIP_TEXT),
            Self::Png(p) => (p.len(), MAX_CLIP_PNG),
        };
        if len > max {
            return Err(LinkError::TooBig { len, max });
        }
        Ok(())
    }
}

/// Send `item` on a new `Clipboard` lane. Refuses oversize items before
/// sending anything; succeeds once the receiver confirms it accepted the clip.
pub fn send_clip(conn: &Connection, item: &ClipItem) -> Result<(), LinkError> {
    item.check()?;
    let (kind, bytes) = match item {
        ClipItem::Text(t) => (ClipKind::Text, t.as_bytes()),
        ClipItem::Png(p) => (ClipKind::Png, p.as_slice()),
    };
    let mut stream = open_lane(conn, LaneKind::Clipboard)?;
    stream.send(&ClipHeader {
        kind,
        len: bytes.len() as u64,
    })?;
    // Slices, so each wait is bounded per MiB rather than for the whole clip.
    for slice in bytes.chunks(SLICE) {
        stream.write_all(slice, CLIP_TIMEOUT)?;
    }
    stream.recv::<()>(CLIP_TIMEOUT)?;
    Ok(())
}

/// Receive one clip on an accepted `Clipboard` lane. The declared length is
/// checked against the cap before any payload is read; text must be UTF-8 and
/// images must carry the PNG signature.
pub fn recv_clip(mut stream: Stream) -> Result<ClipItem, LinkError> {
    let ClipHeader { kind, len } = stream.recv(CLIP_TIMEOUT)?;
    let max = kind.max();
    let len = usize::try_from(len)
        .ok()
        .filter(|&len| len <= max)
        .ok_or(LinkError::TooBig {
            len: usize::try_from(len).unwrap_or(usize::MAX),
            max,
        })?;
    let mut bytes = vec![0; len];
    for slice in bytes.chunks_mut(SLICE) {
        stream.read_exact(slice, CLIP_TIMEOUT)?;
    }
    let item = match kind {
        ClipKind::Text => ClipItem::Text(
            String::from_utf8(bytes).map_err(|_| LinkError::Protocol("clip text is not UTF-8"))?,
        ),
        ClipKind::Png if bytes.starts_with(&PNG_SIGNATURE) => ClipItem::Png(bytes),
        ClipKind::Png => return Err(LinkError::Protocol("clip image is not a PNG")),
    };
    stream.send(&())?;
    stream.finish()?;
    Ok(item)
}

impl fmt::Debug for ClipItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(t) => write!(f, "Text({} bytes)", t.len()),
            Self::Png(p) => write!(f, "Png({} bytes)", p.len()),
        }
    }
}

/// The 20 most recent received clips, newest first.
#[derive(Debug, Default)]
pub struct ClipHistory(VecDeque<ClipItem>);

impl ClipHistory {
    /// Records `item` as the newest clip, dropping the oldest past the cap.
    pub fn push(&mut self, item: ClipItem) {
        self.0.push_front(item);
        self.0.truncate(HISTORY_CAP);
    }

    /// Clips, newest first.
    pub fn items(&self) -> impl Iterator<Item = &ClipItem> {
        self.0.iter()
    }

    /// Forgets every clip.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}
