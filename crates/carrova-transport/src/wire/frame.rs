//! Control frame encoding: length prefix + CBOR body.

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::MAX_FRAME_LEN;
use crate::error::{ProtocolViolation, Result, TransportError};

/// Length prefix of a control frame.
pub(super) const FRAME_HEADER_LEN: usize = 4;

/// Encode `msg` as one control frame.
pub(crate) fn encode_frame<T: Serialize + ?Sized>(msg: &T) -> Result<Vec<u8>> {
    let mut out = vec![0u8; FRAME_HEADER_LEN];
    ciborium::into_writer(msg, &mut out)
        .map_err(|e| TransportError::from(ProtocolViolation::Unencodable(e.to_string())))?;
    let len = out.len() - FRAME_HEADER_LEN;
    let prefix = u32::try_from(len)
        .ok()
        .filter(|_| len <= MAX_FRAME_LEN)
        .ok_or(TransportError::FrameTooLarge { len })?;
    out[..FRAME_HEADER_LEN].copy_from_slice(&prefix.to_be_bytes());
    Ok(out)
}

pub(super) fn decode_body<T: DeserializeOwned>(body: &[u8]) -> Result<T> {
    ciborium::from_reader(body)
        .map_err(|e| TransportError::from(ProtocolViolation::Malformed(e.to_string())))
}

#[cfg(test)]
#[path = "../../tests/wire/frame/unit.rs"]
mod tests;
