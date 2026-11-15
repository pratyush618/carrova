//! Clipboard items, their size limits and the receive-side history.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::LinkError;

/// Largest text clip accepted, in bytes.
pub const MAX_CLIP_TEXT: usize = 1 << 20;
/// Largest PNG clip accepted, in bytes.
pub const MAX_CLIP_PNG: usize = 10 << 20;
/// Clips kept in [`ClipHistory`].
const HISTORY_CAP: usize = 20;

/// One clipboard payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
