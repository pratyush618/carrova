//! SMS conversations and their messages.
//!
//! A message with `id` 0 is provisional: the phone has not stored it yet.
//! The engine puts a thread's provisional texts on top of its newest page
//! and drops each once its stored copy arrives; these views only mark them
//! and never page from them.

use std::cmp::Reverse;

use carrova_link::{MESSAGE_PAGE, THREAD_PAGE};
use carrova_platform::phone::{Cursor, SmsMessage, SmsThread};
use serde::Serialize;

use super::LocalClock;

/// One conversation in the list. `Debug` prints no content.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ThreadRow {
    /// Thread id on the phone.
    pub id: i64,
    /// Contact name, resolved on the phone.
    pub name: Option<String>,
    /// The other party's number.
    pub address: String,
    /// Latest message text, provisional ones included.
    pub snippet: String,
    /// Latest message time (ms since the Unix epoch).
    pub when_ms: i64,
    /// Calendar days back: `0` shows the time, `1` "Yesterday", more the date.
    pub days_ago: u32,
    /// Unread messages.
    pub unread: u32,
}

/// A page of conversations, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ThreadsView {
    /// Newest first.
    pub threads: Vec<ThreadRow>,
    /// Where the next (older) page starts; `None` on the last page.
    pub next: Option<Cursor>,
}

/// One message in a conversation. `Debug` prints no content.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MessageRow {
    /// Message id on the phone; `0` while provisional.
    pub id: i64,
    /// Sent from the phone (rather than received).
    pub outgoing: bool,
    /// Message text.
    pub text: String,
    /// When it was sent or received (ms since the Unix epoch).
    pub when_ms: i64,
    /// Not stored on the phone yet: shown, replaced once stored.
    pub provisional: bool,
}

/// A page of one conversation, oldest first (chat order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ThreadView {
    /// Oldest first.
    pub messages: Vec<MessageRow>,
    /// Where the next (older) page starts; `None` on the last page.
    pub next: Option<Cursor>,
}

private_debug!(ThreadRow, MessageRow);

/// The list view of `threads` (a page, newest first) at `clock`. Each
/// provisional text of `incoming` newer than its thread's latest message
/// leads that thread (snippet, time, unread) until the phone stores it;
/// others (stored, older, or of a thread not listed) change nothing.
#[must_use]
pub fn threads_view(
    threads: &[SmsThread],
    incoming: &[SmsMessage],
    clock: LocalClock,
) -> ThreadsView {
    let next = threads
        .last()
        .filter(|_| threads.len() >= THREAD_PAGE)
        .map(|t| Cursor {
            when_ms: t.when_ms,
            id: t.id,
        });
    let mut rows: Vec<ThreadRow> = threads
        .iter()
        .map(|t| thread_row(t, incoming, clock))
        .collect();
    rows.sort_by_key(|r| Reverse(r.when_ms));
    ThreadsView {
        threads: rows,
        next,
    }
}

fn thread_row(thread: &SmsThread, incoming: &[SmsMessage], clock: LocalClock) -> ThreadRow {
    let newer: Vec<&SmsMessage> = incoming
        .iter()
        .filter(|m| provisional(m) && m.thread_id == thread.id && m.when_ms > thread.when_ms)
        .collect();
    let (snippet, when_ms) = newer
        .iter()
        .max_by_key(|m| m.when_ms)
        .map_or((&thread.snippet, thread.when_ms), |m| (&m.text, m.when_ms));
    let unread = newer.iter().filter(|m| !m.outgoing).count();
    ThreadRow {
        id: thread.id,
        name: thread.name.clone(),
        address: thread.address.clone(),
        snippet: snippet.clone(),
        when_ms,
        days_ago: clock.days_ago(when_ms),
        unread: thread
            .unread
            .saturating_add(u32::try_from(unread).unwrap_or(u32::MAX)),
    }
}

/// The view of `messages`, a page of one thread as the engine returns it
/// (newest first, provisional texts included).
#[must_use]
pub fn thread_view(messages: &[SmsMessage]) -> ThreadView {
    let stored: Vec<Cursor> = messages
        .iter()
        .filter(|m| !provisional(m))
        .map(|m| Cursor {
            when_ms: m.when_ms,
            id: m.id,
        })
        .collect();
    let next = stored
        .iter()
        .min()
        .copied()
        .filter(|_| stored.len() >= MESSAGE_PAGE);
    let mut rows: Vec<MessageRow> = messages
        .iter()
        .rev()
        .map(|m| MessageRow {
            id: m.id,
            outgoing: m.outgoing,
            text: m.text.clone(),
            when_ms: m.when_ms,
            provisional: provisional(m),
        })
        .collect();
    rows.sort_by_key(|r| r.when_ms);
    ThreadView {
        messages: rows,
        next,
    }
}

/// Not stored on the phone yet: never a cursor or a dedupe key.
fn provisional(message: &SmsMessage) -> bool {
    message.id == 0
}
