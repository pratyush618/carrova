//! In-memory `FileSink` that records what the protocol did to it.

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use carrova_link::FileSink;

/// Everything a [`MemSink`] saw.
#[derive(Default, Debug)]
pub struct Seen {
    /// Bytes as written, indexed by offset.
    pub data: Vec<u8>,
    /// Offset of every `write_at`.
    pub writes: Vec<u64>,
    pub committed: bool,
    pub aborted: bool,
}

/// Sink writing into shared memory; the test keeps the other handle.
pub struct MemSink(pub Arc<Mutex<Seen>>);

/// A fresh sink and the handle to inspect it.
pub fn mem_sink() -> (Box<dyn FileSink>, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    (Box::new(MemSink(Arc::clone(&seen))), seen)
}

impl FileSink for MemSink {
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
        let mut seen = self.0.lock().unwrap();
        let start = usize::try_from(offset).unwrap();
        let end = start + bytes.len();
        if seen.data.len() < end {
            seen.data.resize(end, 0);
        }
        seen.data[start..end].copy_from_slice(bytes);
        seen.writes.push(offset);
        Ok(())
    }

    fn commit(self: Box<Self>) -> io::Result<PathBuf> {
        self.0.lock().unwrap().committed = true;
        Ok(PathBuf::from("mem"))
    }

    fn abort(self: Box<Self>) {
        self.0.lock().unwrap().aborted = true;
    }
}
