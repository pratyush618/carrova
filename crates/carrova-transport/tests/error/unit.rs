use carrova_manifest::MAX_CHUNK_LEN;
use quinn::VarInt;

use super::*;
use crate::wire::chunk_len;

/// Display snapshot: error texts reach users and logs unchanged.
#[test]
fn error_texts_are_stable() {
    let code = VarInt::from_u32(0);
    let cases = [
        (
            TransportError::read(&ReadError::ClosedStream),
            "protocol error: stream unexpectedly closed".to_owned(),
        ),
        (
            TransportError::read(&ReadError::Reset(code)),
            "protocol error: peer reset the stream".to_owned(),
        ),
        (
            TransportError::write(&WriteError::Stopped(code)),
            "protocol error: peer stopped the stream".to_owned(),
        ),
        (
            TransportError::read_exact(&ReadExactError::FinishedEarly(0)),
            "protocol error: stream ended mid-message".to_owned(),
        ),
        (
            TransportError::internal("connect", "boom"),
            "transport setup failed: connect: boom".to_owned(),
        ),
        (TransportError::Cancelled, "cancelled".to_owned()),
        (
            TransportError::PeerCancelled,
            "the other machine cancelled the session".to_owned(),
        ),
        (
            chunk_len(MAX_CHUNK_LEN + 1).unwrap_err(),
            format!(
                "protocol error: chunk of {} bytes exceeds the {MAX_CHUNK_LEN}-byte limit",
                MAX_CHUNK_LEN + 1
            ),
        ),
    ];
    for (err, text) in cases {
        assert_eq!(err.to_string(), text);
    }
}

/// Display snapshot of every typed reason: the texts the transport
/// produced as strings before the reasons were typed.
#[test]
fn typed_reasons_keep_their_texts() {
    use ProtocolViolation as V;
    use SetupFailure as S;
    let hash = ChunkHash::of(b"x");
    let violations = [
        (
            V::PeerClosed,
            "peer closed the connection: protocol violation".to_owned(),
        ),
        (V::StreamReset, "peer reset the stream".to_owned()),
        (V::StreamStopped, "peer stopped the stream".to_owned()),
        (V::StreamClosed, "stream unexpectedly closed".to_owned()),
        (V::TruncatedMessage, "stream ended mid-message".to_owned()),
        (
            V::Malformed("eof".into()),
            "malformed message: eof".to_owned(),
        ),
        (
            V::Unencodable("io".into()),
            "encoding a message failed: io".to_owned(),
        ),
        (
            V::UnexpectedHandshake { wanted: "welcome" },
            "unexpected handshake message (wanted welcome)".to_owned(),
        ),
        (
            V::IncompatibleVersion,
            "the sender runs an incompatible Carrova version".to_owned(),
        ),
        (
            V::NoPeerCertificate,
            "peer presented no certificate".to_owned(),
        ),
        (V::UnknownRecordKind, "unknown chunk record kind".to_owned()),
        (V::ChunkLengthOverflow, "chunk length overflow".to_owned()),
        (
            V::ChunkStreamEnded,
            "chunk stream ended before all requested chunks".to_owned(),
        ),
        (
            V::UnrequestedChunk,
            "sender answered with a chunk that was not requested next".to_owned(),
        ),
        (
            V::ChunkLengthMismatch,
            "chunk length differs from the manifest".to_owned(),
        ),
        (
            V::FetchDoneData,
            "unexpected data on the fetch-done stream".to_owned(),
        ),
        (V::FetchEndedTwice, "fetch ended twice".to_owned()),
        (
            V::NotInManifest(hash),
            format!("receiver requested chunk {hash}, which is not in the manifest"),
        ),
        (
            V::OverRequested,
            "receiver requested more than 3× the manifest's chunk bytes".to_owned(),
        ),
    ];
    for (v, text) in violations {
        assert_eq!(
            TransportError::from(v).to_string(),
            format!("protocol error: {text}")
        );
    }
    let setup = [
        (S::ExporterUnavailable, "TLS exporter unavailable"),
        (S::HandshakeStateMismatch, "handshake state mismatch"),
        (S::EndpointClosed, "endpoint closed"),
        (S::NotIpv4, "endpoint is not bound to IPv4"),
        (S::ServingStopped, "serving was stopped"),
        (S::DriverJoined, "driver already joined"),
    ];
    for (f, text) in setup {
        assert_eq!(
            TransportError::from(f).to_string(),
            format!("transport setup failed: {text}")
        );
    }
}
