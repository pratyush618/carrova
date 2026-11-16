//! What a linked phone reports and is asked to do: notifications, SMS,
//! calls and the Android permissions behind them. Plain data, defined once
//! here so the Android adapter, the link lanes and the views share it.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A notification posted on the phone.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Note {
    /// Android notification key; identifies it for removal and actions.
    pub key: String,
    /// Package id of the posting app.
    pub app_id: String,
    /// The app's display name.
    pub app_name: String,
    /// Notification title.
    pub title: String,
    /// Notification text (empty when hidden).
    pub text: String,
    /// When it was posted (ms since the Unix epoch).
    pub when_ms: i64,
    /// Whether it offers an inline reply.
    pub can_reply: bool,
    /// App icon as PNG bytes, when small enough to send.
    // A CBOR byte string, not an int array (which would double its size).
    #[serde(with = "serde_bytes", default)]
    #[cfg_attr(feature = "ts", ts(type = "Array<number> | null"))]
    pub icon: Option<Vec<u8>>,
    /// Why some of it was withheld; `None` when shown in full.
    pub hidden: Option<Hidden>,
}

/// Why a note's content was withheld (set by core filters; the UI picks the copy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Hidden {
    /// It carried a one-time code.
    Code,
    /// Message text is switched off: title only.
    TextOff,
    /// This computer is locked: only a count shows.
    Locked,
}

/// One SMS conversation.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SmsThread {
    /// Thread id on the phone.
    pub id: i64,
    /// Contact name, resolved on the phone.
    pub name: Option<String>,
    /// The other party's number.
    pub address: String,
    /// Latest message text.
    pub snippet: String,
    /// Latest message time (ms since the Unix epoch).
    pub when_ms: i64,
    /// Unread messages.
    pub unread: u32,
}

/// One SMS message.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SmsMessage {
    /// Message id on the phone.
    pub id: i64,
    /// The thread it belongs to.
    pub thread_id: i64,
    /// Sent from the phone (rather than received).
    pub outgoing: bool,
    /// Message text.
    pub text: String,
    /// When it was sent or received (ms since the Unix epoch).
    pub when_ms: i64,
}

/// The phone's call, as it changes.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CallState {
    /// No call.
    Idle,
    /// An incoming call is ringing.
    Ringing {
        /// Caller's number.
        number: String,
        /// Caller's contact name, resolved on the phone.
        name: Option<String>,
    },
    /// A call is in progress.
    Active {
        /// When it was answered (ms since the Unix epoch).
        since_ms: i64,
        /// The microphone is muted.
        muted: bool,
        /// Audio runs through this computer.
        on_laptop_audio: bool,
    },
    /// The call ended.
    Ended {
        /// It rang out unanswered.
        missed: bool,
    },
}

/// One entry of the phone's call log.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CallRecord {
    /// The other party's number.
    pub number: String,
    /// Contact name, resolved on the phone.
    pub name: Option<String>,
    /// When the call started (ms since the Unix epoch).
    pub when_ms: i64,
    /// How long it lasted.
    pub duration_s: u32,
    /// Its direction and outcome.
    pub kind: CallKind,
}

/// A logged call's direction and outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum CallKind {
    /// Answered incoming call.
    Incoming,
    /// Outgoing call.
    Outgoing,
    /// Incoming call nobody answered.
    Missed,
    /// Incoming call that was declined.
    Declined,
}

/// Something the phone reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum PhoneEvent {
    /// A notification was posted or updated.
    NotePosted(Note),
    /// A notification went away.
    NoteRemoved {
        /// Its key.
        key: String,
    },
    /// An SMS arrived.
    SmsReceived(SmsMessage),
    /// The call state changed.
    Call(CallState),
    /// The phone's event queue overflowed and dropped `count` events;
    /// the receiver resends its snapshot.
    Dropped {
        /// Events lost.
        count: u32,
    },
}

/// How sending an SMS went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
// The TS name `SendOutcome` is taken by the migration send report.
#[cfg_attr(feature = "ts", ts(rename = "SmsSendOutcome"))]
pub enum SendOutcome {
    /// Sent.
    Sent,
    /// The phone has no signal.
    NoSignal,
    /// Sending failed.
    Failed,
}

/// Where a call's audio plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum AudioSide {
    /// On the phone.
    Phone,
    /// On this computer.
    Laptop,
}

/// An Android permission a phone feature needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum PhonePerm {
    /// Notification listener access.
    NotificationAccess,
    /// Read and send SMS.
    Sms,
    /// Phone state, call log, answer and place calls.
    Calls,
    /// Contact names.
    Contacts,
    /// Bluetooth connection, for call audio.
    BluetoothConnect,
}

/// Whether a [`PhonePerm`] is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum PermState {
    /// Granted.
    Granted,
    /// Not granted; can be asked for.
    Denied,
    /// Blocked by Android's restricted settings until the user allows them.
    Restricted,
}

// Debug for the types that carry personal content prints ids, flags and byte
// lengths only: text, names and numbers never reach a log.

/// A field standing in for private text: its length only.
struct Len(usize);

impl fmt::Debug for Len {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{} bytes>", self.0)
    }
}

fn len(s: &str) -> Len {
    Len(s.len())
}

fn opt_len(s: Option<&String>) -> Option<Len> {
    s.map(|s| Len(s.len()))
}

impl fmt::Debug for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Note")
            .field("key", &len(&self.key))
            .field("app_id", &len(&self.app_id))
            .field("app_name", &len(&self.app_name))
            .field("title", &len(&self.title))
            .field("text", &len(&self.text))
            .field("can_reply", &self.can_reply)
            .field("icon", &self.icon.as_deref().map(|i| Len(i.len())))
            .field("hidden", &self.hidden)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for SmsThread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmsThread")
            .field("id", &self.id)
            .field("name", &opt_len(self.name.as_ref()))
            .field("address", &len(&self.address))
            .field("snippet", &len(&self.snippet))
            .field("unread", &self.unread)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for SmsMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmsMessage")
            .field("id", &self.id)
            .field("thread_id", &self.thread_id)
            .field("outgoing", &self.outgoing)
            .field("text", &len(&self.text))
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for CallState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => f.write_str("Idle"),
            Self::Ringing { number, name } => f
                .debug_struct("Ringing")
                .field("number", &len(number))
                .field("name", &opt_len(name.as_ref()))
                .finish(),
            Self::Active {
                muted,
                on_laptop_audio,
                ..
            } => f
                .debug_struct("Active")
                .field("muted", muted)
                .field("on_laptop_audio", on_laptop_audio)
                .finish_non_exhaustive(),
            Self::Ended { missed } => f.debug_struct("Ended").field("missed", missed).finish(),
        }
    }
}

impl fmt::Debug for CallRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallRecord")
            .field("number", &len(&self.number))
            .field("name", &opt_len(self.name.as_ref()))
            .field("duration_s", &self.duration_s)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}
