//! File offer and transfer over a `File` lane (public API).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod sink;
#[path = "../support/mod.rs"]
mod support;

use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::thread;

use carrova_link::{
    FileOffer, LaneKind, LinkError, OfferAnswer, accept_lane, offer_file, open_lane, receive_file,
};
use carrova_transport::Connection;
use serde::Serialize;
use sink::mem_sink;
use support::{Open, T, linked};

const MIB: usize = 1 << 20;

/// Deterministic, non-repeating-per-chunk test bytes.
fn bytes(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 31 % 251) as u8).collect()
}

fn offer(name: &str, size: usize) -> FileOffer {
    FileOffer {
        id: 7,
        name: name.into(),
        size: size as u64,
        mime: Some("application/pdf".into()),
    }
}

/// Run `receive_file` on `b`'s next lane with `answer`, recording the offer it was shown.
fn receiver(
    b: &Connection,
    answer: OfferAnswer,
    sink: Box<dyn carrova_link::FileSink>,
) -> (Result<PathBuf, LinkError>, Option<FileOffer>, Vec<u64>) {
    let (kind, stream) = accept_lane(b, &Open, T).unwrap();
    assert_eq!(kind, LaneKind::File);
    let mut shown = None;
    let mut progress = Vec::new();
    let got = receive_file(
        stream,
        &mut |o| {
            shown = Some(o.clone());
            answer
        },
        sink,
        &mut |n| progress.push(n),
    );
    (got, shown, progress)
}

#[test]
fn file_roundtrip_hash_matches() {
    let (a, b) = linked();
    let data = bytes(2 * MIB + 123);
    let (sink, seen) = mem_sink();
    thread::scope(|s| {
        let recv = s.spawn(|| receiver(&b, OfferAnswer::Accept { resume_from: 0 }, sink));
        let mut sent = Vec::new();
        offer_file(
            &a,
            &offer("../../etc/report.pdf", data.len()),
            &mut Cursor::new(&data),
            &mut |n| sent.push(n),
        )
        .unwrap();
        let (got, shown, progress) = recv.join().unwrap();
        assert_eq!(got.unwrap(), PathBuf::from("mem"));
        // The decision callback only ever sees the normalised name.
        assert_eq!(shown.unwrap().name, "etcreport.pdf");
        assert_eq!(sent.last(), Some(&(data.len() as u64)));
        assert_eq!(progress.last(), Some(&(data.len() as u64)));
    });
    let seen = seen.lock().unwrap();
    assert!(seen.committed && !seen.aborted);
    assert_eq!(blake3::hash(&seen.data), blake3::hash(&data));
    assert_eq!(seen.writes, [0, MIB as u64, 2 * MIB as u64]);
}

#[test]
fn declined_offer_writes_nothing() {
    for (answer, want) in [
        (OfferAnswer::Decline, "Declined"),
        (OfferAnswer::Expired, "Expired"),
    ] {
        let (a, b) = linked();
        let data = bytes(1000);
        let (sink, seen) = mem_sink();
        thread::scope(|s| {
            let recv = s.spawn(|| receiver(&b, answer, sink));
            let sent = offer_file(
                &a,
                &offer("x", data.len()),
                &mut Cursor::new(&data),
                &mut |_| {},
            );
            let (got, _, progress) = recv.join().unwrap();
            assert!(format!("{:?}", sent.unwrap_err()).starts_with(want));
            assert!(format!("{:?}", got.unwrap_err()).starts_with(want));
            assert!(progress.is_empty());
        });
        let seen = seen.lock().unwrap();
        assert!(seen.writes.is_empty() && !seen.committed && seen.aborted);
    }
}

/// Mirror of the private `File` lane frames: the wire format is the protocol.
#[derive(Serialize)]
enum FileFrame {
    Chunk { len: u32, hash: [u8; 32] },
    Done,
}

#[test]
fn corrupt_chunk_fails() {
    let (a, b) = linked();
    let data = bytes(4096);
    let (sink, seen) = mem_sink();
    thread::scope(|s| {
        let recv = s.spawn(|| receiver(&b, OfferAnswer::Accept { resume_from: 0 }, sink));
        let mut lane = open_lane(&a, LaneKind::File).unwrap();
        lane.send(&offer("x", data.len())).unwrap();
        let _: OfferAnswer = lane.recv(T).unwrap();
        let hash = *blake3::hash(&data).as_bytes();
        let mut corrupt = data.clone();
        corrupt[100] ^= 1;
        lane.send(&FileFrame::Chunk { len: 4096, hash }).unwrap();
        lane.write_all(&corrupt, T).unwrap();
        lane.send(&FileFrame::Done).unwrap();
        let (got, _, _) = recv.join().unwrap();
        assert!(matches!(got, Err(LinkError::Protocol(_))));
    });
    let seen = seen.lock().unwrap();
    assert!(seen.writes.is_empty() && !seen.committed && seen.aborted);
}

#[test]
fn oversized_or_extra_chunks_fail() {
    for (len, total) in [(MIB as u32 + 1, MIB + 1), (10, 5)] {
        let (a, b) = linked();
        let data = bytes(len as usize);
        let (sink, seen) = mem_sink();
        thread::scope(|s| {
            let recv = s.spawn(|| receiver(&b, OfferAnswer::Accept { resume_from: 0 }, sink));
            let mut lane = open_lane(&a, LaneKind::File).unwrap();
            lane.send(&offer("x", total)).unwrap();
            let _: OfferAnswer = lane.recv(T).unwrap();
            let hash = *blake3::hash(&data).as_bytes();
            // The receiver may stop reading at the bad header; later writes can fail.
            let _ = lane.send(&FileFrame::Chunk { len, hash });
            let _ = lane.write_all(&data, T);
            let _ = lane.send(&FileFrame::Done);
            let (got, _, _) = recv.join().unwrap();
            assert!(matches!(got, Err(LinkError::Protocol(_))), "len {len}");
        });
        let seen = seen.lock().unwrap();
        assert!(seen.writes.is_empty() && !seen.committed && seen.aborted);
    }
}

/// Reader that fails once `ok` bytes have been read.
struct Failing {
    inner: Cursor<Vec<u8>>,
    ok: u64,
}

impl Read for Failing {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.ok.saturating_sub(self.inner.position());
        if left == 0 {
            return Err(io::Error::other("disk gone"));
        }
        let n = buf.len().min(usize::try_from(left).unwrap());
        self.inner.read(&mut buf[..n])
    }
}

impl Seek for Failing {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

#[test]
fn interrupted_transfer_leaves_no_final_file() {
    let (a, b) = linked();
    let data = bytes(3 * MIB);
    let (sink, seen) = mem_sink();
    thread::scope(|s| {
        let recv = s.spawn(|| receiver(&b, OfferAnswer::Accept { resume_from: 0 }, sink));
        let mut src = Failing {
            inner: Cursor::new(data.clone()),
            ok: (MIB + MIB / 2) as u64,
        };
        let sent = offer_file(&a, &offer("x", data.len()), &mut src, &mut |_| {});
        // The sender's lane is dropped (which finishes it) without an end frame.
        assert!(matches!(sent, Err(LinkError::Io(_))));
        let (got, _, _) = recv.join().unwrap();
        assert!(got.is_err());
    });
    let seen = seen.lock().unwrap();
    assert_eq!(seen.writes, [0]);
    assert!(!seen.committed && seen.aborted);
}

#[test]
fn resume_from_offset() {
    let (a, b) = linked();
    let data = bytes(2 * MIB);
    let from = 700_000_u64;
    let (sink, seen) = mem_sink();
    thread::scope(|s| {
        let recv = s.spawn(|| receiver(&b, OfferAnswer::Accept { resume_from: from }, sink));
        offer_file(
            &a,
            &offer("x", data.len()),
            &mut Cursor::new(&data),
            &mut |_| {},
        )
        .unwrap();
        let (got, _, progress) = recv.join().unwrap();
        got.unwrap();
        assert_eq!(progress.last(), Some(&(data.len() as u64)));
    });
    let seen = seen.lock().unwrap();
    assert!(seen.committed);
    assert_eq!(seen.writes, [from, from + MIB as u64]);
    let start = usize::try_from(from).unwrap();
    assert_eq!(&seen.data[start..], &data[start..]);
}

#[test]
fn resume_past_end_is_refused() {
    let (a, b) = linked();
    let data = bytes(10);
    let (sink, seen) = mem_sink();
    thread::scope(|s| {
        let recv = s.spawn(|| receiver(&b, OfferAnswer::Accept { resume_from: 11 }, sink));
        let sent = offer_file(
            &a,
            &offer("x", data.len()),
            &mut Cursor::new(&data),
            &mut |_| {},
        );
        let (got, _, _) = recv.join().unwrap();
        assert!(matches!(got, Err(LinkError::Protocol(_))));
        assert!(sent.is_err());
    });
    let seen = seen.lock().unwrap();
    assert!(seen.writes.is_empty() && !seen.committed && seen.aborted);
}
