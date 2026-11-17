//! Carrova link: per-device feature policy and the two-sided gate.

mod calls;
mod clipboard;
mod error;
mod file;
mod handoff;
mod lane;
mod messages;
mod name;
mod notify;
mod ping;
mod policy;
mod reply;
mod ring;

pub use calls::{
    CallCommand, CallIncoming, CallReply, HISTORY_PAGE, call_command, recv_calls, send_call_state,
};
pub use carrova_platform::phone::{AudioSide, SendOutcome};
pub use clipboard::{
    ClipHistory, ClipItem, MAX_CLIP_PNG, MAX_CLIP_TEXT, ReceivedClip, recv_clip, send_clip,
};
pub use error::LinkError;
pub use file::{
    ANSWER_TIMEOUT, FileOffer, FileSink, OfferAnswer, ReadSeek, offer_file, receive_file,
};
pub use handoff::{Handoff, ReceivedHandoff, SafeUrl, check_url, recv_handoff, send_handoff};
pub use lane::{LaneGate, LaneKind, LaneOpen, LaneReply, RefuseReason, accept_lane, open_lane};
pub use messages::{
    MAX_SIMS, MAX_SMS_TEXT, MESSAGE_PAGE, SmsIncoming, SmsPush, SmsReply, SmsRequest, THREAD_PAGE,
    check_sms_reply, push_sms, recv_sms, request_sms,
};
pub use name::safe_file_name;
pub use notify::{
    ActionResult, AppEntry, Incoming, MAX_APP_NAME, MAX_APPS, MAX_ICON, MAX_NOTE_TEXT,
    MAX_NOTE_TITLE, MAX_REPLY, MAX_SNAPSHOT, NotifyAction, NotifyMsg, fit_apps, fit_note,
    fit_snapshot, list_apps, recv_notify, send_action, send_notify,
};
pub use ping::{ping, serve_ping};
pub use policy::{
    Feature, FeatureState, Group, PolicyDoc, PolicyUpdate, allowed, recv_policy, send_policy,
};
pub use reply::{Pending, Responder, request, serve};
pub use ring::{RING_SECONDS, RingMsg, recv_ring, send_ring};
