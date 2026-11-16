//! What a linked phone reports and is asked to do: notifications, SMS,
//! calls and the Android permissions behind them. Plain data, defined once
//! here so the Android adapter, the link lanes and the views share it.

use serde::{Deserialize, Serialize};

/// A notification posted on the phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
