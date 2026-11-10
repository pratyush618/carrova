//! Wire format.
//!
//! - **Control frames** (handshake, application messages, chunk requests):
//!   `u32` big-endian body length + CBOR body, at most [`MAX_FRAME_LEN`].
//!   Byte fields use `serde_bytes` (CBOR byte strings, not integer arrays).
//! - **Chunk records** (sender → receiver on chunk streams): a fixed binary
//!   header `kind: u8 | len: u32 BE | hash: [u8; 32]` followed by `len` raw
//!   bytes for [`RecordKind::Data`]. Never CBOR.
//! - **Close codes** ([`CloseCode`]) tell the peer why a connection ended.
//!
//! [`MAX_FRAME_LEN`]: crate::MAX_FRAME_LEN

mod close;
mod frame;
mod messages;
mod reader;
mod record;

pub(crate) use close::{CloseCode, STREAM_CANCELLED};
pub(crate) use frame::encode_frame;
pub(crate) use messages::{ChunkRequest, Handshake, Intent, RejectReason, StreamOpen};
pub(crate) use reader::StreamReader;
pub(crate) use record::{RECORD_HEADER_LEN, RecordHeader, RecordKind, chunk_len};
