//! Handshake frame I/O on the control stream.

use carrova_crypto::pairing::Confirmation;
use quinn::SendStream;

use crate::error::{ProtocolViolation, Result, TransportError};
use crate::wire::{Handshake, StreamReader, encode_frame};

pub(super) async fn write(send: &mut SendStream, msgs: &[Handshake]) -> Result<()> {
    let mut buf = Vec::new();
    for m in msgs {
        buf.extend(encode_frame(m)?);
    }
    send.write_all(&buf)
        .await
        .map_err(|e| TransportError::write(&e))
}

pub(super) fn unexpected(wanted: &'static str) -> TransportError {
    ProtocolViolation::UnexpectedHandshake { wanted }.into()
}

pub(super) async fn read_confirm(recv: &mut StreamReader) -> Result<Confirmation> {
    match recv.expect_frame().await? {
        Handshake::Confirm(c) => Ok(Confirmation(c)),
        _ => Err(unexpected("confirmation")),
    }
}
