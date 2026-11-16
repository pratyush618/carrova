//! The phone's side of notifications, SMS and calls: forwards what the
//! phone reports to every linked device allowing that feature, resends its
//! state when a device starts taking it or events were dropped, and serves
//! the devices' requests (reply, send a text, answer, ...).
//!
//! One thread does all sending, so a device sees events in order. The
//! phone's events are watched only while some linked device takes a phone
//! feature. Never logs keys, titles, text, app names, numbers or names.

use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use carrova_crypto::identity::DeviceId;
use carrova_link::{
    ActionResult, CallCommand, CallIncoming, CallReply, Feature, FeatureState, HISTORY_PAGE,
    Incoming, LinkError, MESSAGE_PAGE, NotifyAction, NotifyMsg, PolicyDoc, SendOutcome,
    SmsIncoming, SmsPush, SmsReply, SmsRequest, THREAD_PAGE, allowed, check_sms_reply, fit_note,
    fit_snapshot, push_sms, recv_calls, recv_notify, recv_sms, send_call_state, send_notify,
};
use carrova_platform::phone::{CallState, Note, PhoneEvent, PhoneWatch};
use carrova_platform::{Platform, PlatformError};
use carrova_transport::{Connection, Stream};

use super::NotifyApps;
use super::apps::outgoing;
use super::mirror;
use crate::cancel::Cancel;
use crate::ecosystem::supervisor::Shared;
use crate::error::Result;

/// The features a phone gives.
const PHONE: [Feature; 3] = [Feature::Notifications, Feature::Messages, Feature::Calls];

/// How far back SMS pages go ("Keep in sync"), in ms.
// shortcut: fixed at the 30-day default; read the user's choice once the
// phone's Messages screen stores one.
const SYNC_WINDOW_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// How long an answered call may take to report itself active.
const ANSWER_SETTLE: Duration = Duration::from_secs(2);
/// How often the call state is read meanwhile.
const ANSWER_POLL: Duration = Duration::from_millis(50);

/// Work for the giving thread.
#[derive(Debug)]
pub(crate) enum Job {
    /// The phone reported something.
    Event(PhoneEvent),
    /// `device` started taking what `gives` holds: send it the current state.
    Opened { device: DeviceId, gives: Gives },
}

/// Which pushed state a device takes (SMS is pulled, so not here).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Gives {
    notes: bool,
    calls: bool,
}

impl Gives {
    /// What this side gives a device on `local` whose policy is `remote`.
    pub(crate) fn of(local: &PolicyDoc, remote: Option<&PolicyDoc>) -> Self {
        let on = |f| remote.is_some_and(|r| allowed(local, r, f) == FeatureState::On);
        Self {
            notes: on(Feature::Notifications),
            calls: on(Feature::Calls),
        }
    }

    /// What `self` gives that `before` did not.
    pub(crate) fn since(self, before: Self) -> Self {
        Self {
            notes: self.notes && !before.notes,
            calls: self.calls && !before.calls,
        }
    }

    /// Queue the state `self` gives for `device`, if any.
    pub(crate) fn send(self, shared: &Shared, device: DeviceId) {
        if self != Self::default() {
            let _ = shared.give.send(Job::Opened {
                device,
                gives: self,
            });
        }
    }
}

/// Run the giving thread until the supervisor stops.
pub(crate) fn run(shared: &Arc<Shared>, jobs: &Receiver<Job>) {
    let mut giver = Giver {
        shared,
        watch: None,
        tried: Vec::new(),
        sent: HashSet::new(),
    };
    while !shared.stop.is_cancelled() {
        giver.follow();
        match jobs.recv_timeout(shared.config.tick) {
            Ok(Job::Event(event)) => giver.event(event),
            Ok(Job::Opened { device, gives }) => {
                // Watch first: a note posted while the snapshot is taken
                // then arrives as an event, not lost in between.
                giver.follow();
                giver.resend(Some(device), gives);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

type Targets = Vec<(DeviceId, Arc<Connection>)>;

struct Giver<'a> {
    shared: &'a Arc<Shared>,
    watch: Option<PhoneWatch>,
    /// The device features the last watch attempt was for: a failed one is
    /// tried again only once these change, never in a loop.
    tried: Vec<(DeviceId, Feature)>,
    /// Keys of the notes sent, so a removal of one never sent stays here.
    sent: HashSet<String>,
}

impl Giver<'_> {
    fn platform(&self) -> &dyn Platform {
        &**self.shared.platform()
    }

    /// Watch the phone while a linked device takes a phone feature.
    fn follow(&mut self) {
        let want: Vec<(DeviceId, Feature)> = PHONE
            .iter()
            .flat_map(|&f| {
                self.shared
                    .linked_with(f)
                    .into_iter()
                    .map(move |(d, _)| (d, f))
            })
            .collect();
        if want.is_empty() {
            self.watch = None;
            self.tried.clear();
            // Removals go unseen while unwatched: a later one must not
            // withdraw a note sent before.
            self.sent.clear();
            return;
        }
        if self.watch.is_some() || want == self.tried {
            return;
        }
        self.tried = want;
        let jobs = self.shared.give.clone();
        let on_event = Box::new(move |e| {
            let _ = jobs.send(Job::Event(e));
        });
        match self.platform().watch_phone_events(on_event) {
            Ok(watch) => self.watch = Some(watch),
            Err(e) => tracing::debug!(error = %e, "phone events not watched"),
        }
    }

    fn event(&mut self, event: PhoneEvent) {
        match event {
            PhoneEvent::NotePosted(note) => {
                let targets = self.shared.linked_with(Feature::Notifications);
                if targets.is_empty() {
                    return;
                }
                let key = note.key.clone();
                let msg = match self.filtered(vec![note]).pop() {
                    Some(note) => {
                        self.sent.insert(key);
                        NotifyMsg::Posted(fit_note(note))
                    }
                    // Its app was turned off since it was sent: withdraw it.
                    None if self.sent.remove(&key) => NotifyMsg::Removed { key },
                    None => return,
                };
                send(&targets, |c| send_notify(c, &msg));
            }
            PhoneEvent::NoteRemoved { key } => {
                if self.sent.remove(&key) {
                    let msg = NotifyMsg::Removed { key };
                    let targets = self.shared.linked_with(Feature::Notifications);
                    send(&targets, |c| send_notify(c, &msg));
                }
            }
            PhoneEvent::SmsReceived(message) => self.push(&SmsPush::Incoming(message)),
            PhoneEvent::SmsStored { thread_id } => self.push(&SmsPush::Stored { thread_id }),
            PhoneEvent::Call(state) => {
                let targets = self.shared.linked_with(Feature::Calls);
                send(&targets, |c| send_call_state(c, &state));
            }
            PhoneEvent::Dropped { count } => {
                tracing::debug!(count, "phone events dropped; resending state");
                let all = Gives {
                    notes: true,
                    calls: true,
                };
                self.resend(None, all);
                self.push(&SmsPush::Resync);
            }
        }
    }

    fn push(&self, push: &SmsPush) {
        let targets = self.shared.linked_with(Feature::Messages);
        send(&targets, |c| push_sms(c, push));
    }

    /// Send the notes showing and the call state, as `gives` says, to
    /// `only` (or to every device taking them).
    fn resend(&mut self, only: Option<DeviceId>, gives: Gives) {
        let targets = |f| -> Targets {
            let mut t = self.shared.linked_with(f);
            t.retain(|(d, _)| only.is_none_or(|o| o == *d));
            t
        };
        let notes = if gives.notes {
            targets(Feature::Notifications)
        } else {
            Vec::new()
        };
        if !notes.is_empty() {
            match self.platform().active_notes() {
                Ok(active) => {
                    let snapshot = fit_snapshot(self.filtered(active));
                    if only.is_none() {
                        self.sent.clear();
                    }
                    self.sent.extend(snapshot.iter().map(|n| n.key.clone()));
                    let msg = NotifyMsg::Snapshot(snapshot);
                    send(&notes, |c| send_notify(c, &msg));
                }
                Err(e) => tracing::debug!(error = %e, "notifications unreadable"),
            }
        }
        let calls = if gives.calls {
            targets(Feature::Calls)
        } else {
            Vec::new()
        };
        if !calls.is_empty() {
            match self.platform().call_state() {
                Ok(state) => send(&calls, |c| send_call_state(c, &state)),
                Err(e) => tracing::debug!(error = %e, "call state unreadable"),
            }
        }
    }

    /// `notes` as they may leave the phone (none if the app list is
    /// unreadable: fail closed).
    fn filtered(&self, notes: Vec<Note>) -> Vec<Note> {
        match NotifyApps::open(self.platform()) {
            Ok(apps) => notes
                .into_iter()
                .filter_map(|n| outgoing(n, &apps))
                .collect(),
            Err(e) => {
                tracing::warn!(error = %e, "notification apps unreadable; nothing sent");
                Vec::new()
            }
        }
    }
}

/// Send to each target in turn; a failure is logged and skipped.
fn send(targets: &Targets, f: impl Fn(&Connection) -> std::result::Result<(), LinkError>) {
    for (_, conn) in targets {
        if let Err(e) = f(conn) {
            tracing::debug!(error = %e, "phone update not sent");
        }
    }
}

/// Serve a `Notify` lane from `device` on its link `generation`: mirror a
/// phone's message, or carry out the device's action on a notification.
pub(crate) fn serve_notify(
    shared: &Shared,
    device: DeviceId,
    generation: u64,
    stream: Stream,
) -> Result<()> {
    let (action, responder) = match recv_notify(stream)? {
        Incoming::Msg(msg, ack) => {
            mirror::take_notify(shared, device, generation, msg, ack);
            return Ok(());
        }
        Incoming::Action(action, responder) => (action, responder),
    };
    let platform = &**shared.platform();
    let done = match &action {
        NotifyAction::Reply { key, text } => platform.note_reply(key, text).map(|live| {
            if live {
                ActionResult::Done
            } else {
                ActionResult::Gone
            }
        }),
        NotifyAction::Dismiss { key } => platform.note_dismiss(key).map(|()| ActionResult::Done),
        NotifyAction::Open { key } => platform.note_open(key).map(|()| ActionResult::Done),
    };
    let result = done.unwrap_or_else(|e| {
        tracing::debug!(?action, error = %e, "notification action failed");
        ActionResult::Failed
    });
    Ok(responder.reply(&result)?)
}

/// Serve a `Messages` lane from `device` on its link `generation`: pages
/// within the sync window, and texts sent at most once per `client_id`.
pub(crate) fn serve_messages(
    shared: &Arc<Shared>,
    device: DeviceId,
    generation: u64,
    stream: Stream,
) -> Result<()> {
    let (request, responder) = match recv_sms(stream)? {
        SmsIncoming::Push(push, ack) => {
            mirror::take_sms(shared, device, generation, push, ack);
            return Ok(());
        }
        SmsIncoming::Request(request, responder) => (request, responder),
    };
    let platform = &**shared.platform();
    let since = since_ms();
    let reply = match request {
        SmsRequest::Threads { before } => {
            SmsReply::ThreadsPage(platform.sms_threads(before, THREAD_PAGE, Some(since))?)
        }
        SmsRequest::Thread { id, before } => {
            let mut page = platform.sms_thread(id, before, MESSAGE_PAGE)?;
            // Newest first: what is cut is older than the rest.
            page.retain(|m| m.when_ms > since);
            SmsReply::ThreadPage(page)
        }
        SmsRequest::Send {
            to,
            text,
            client_id,
        } => {
            let first = shared.with_link(&device, generation, |l| {
                match l.sms_sent.entry(client_id) {
                    // Still sending: it may yet go out.
                    Entry::Occupied(o) => Some(o.get().unwrap_or(SendOutcome::Unknown)),
                    Entry::Vacant(v) => {
                        v.insert(None);
                        None
                    }
                }
            });
            let outcome = match first {
                // The link closed: the answer could not arrive, so do not send.
                None => SendOutcome::Failed,
                Some(Some(again)) => again,
                Some(None) => {
                    let outcome = platform.sms_send(&to, &text).unwrap_or_else(|e| {
                        tracing::debug!(error = %e, "text not sent");
                        SendOutcome::Failed
                    });
                    shared.with_link(&device, generation, |l| {
                        l.sms_sent.insert(client_id, Some(outcome));
                    });
                    outcome
                }
            };
            SmsReply::SendResult { client_id, outcome }
        }
    };
    check_sms_reply(&reply)?;
    Ok(responder.reply(&reply)?)
}

/// The oldest time SMS pages reach back to.
fn since_ms() -> i64 {
    let now = i64::try_from(crate::time::now_unix()).unwrap_or(i64::MAX);
    now.saturating_mul(1000).saturating_sub(SYNC_WINDOW_MS)
}

/// Once the answered call is active, whether its audio runs on the laptop;
/// `None` if it is not active within [`ANSWER_SETTLE`] or `stop` fires.
fn answered_on_laptop(p: &dyn Platform, stop: &Cancel) -> Option<bool> {
    let until = Instant::now() + ANSWER_SETTLE;
    loop {
        if let Ok(CallState::Active {
            on_laptop_audio, ..
        }) = p.call_state()
        {
            return Some(on_laptop_audio);
        }
        if Instant::now() >= until || stop.sleep(ANSWER_POLL).is_err() {
            return None;
        }
    }
}

/// Serve a `Calls` lane from `device` on its link `generation`: mirror a
/// phone's call state, or carry out the device's call command.
pub(crate) fn serve_calls(
    shared: &Shared,
    device: DeviceId,
    generation: u64,
    stream: Stream,
) -> Result<()> {
    let (command, responder) = match recv_calls(stream)? {
        CallIncoming::State(state, ack) => {
            mirror::take_call(shared, device, generation, state, ack);
            return Ok(());
        }
        CallIncoming::Command(command, responder) => (command, responder),
    };
    // The stop ends a wait for an answered call.
    let reply = call(&**shared.platform(), &shared.stop, &command).unwrap_or_else(|e| {
        tracing::debug!(?command, error = %e, "call command failed");
        CallReply::Failed
    });
    Ok(responder.reply(&reply)?)
}

fn call(
    p: &dyn Platform,
    stop: &Cancel,
    command: &CallCommand,
) -> std::result::Result<CallReply, PlatformError> {
    // Refuse a command the call cannot take before touching it.
    if let CallCommand::Answer | CallCommand::Decline | CallCommand::End = command {
        let state = p.call_state()?;
        let ringing = matches!(state, CallState::Ringing { .. });
        let live = ringing || matches!(state, CallState::Active { .. });
        if !ringing && !matches!(command, CallCommand::End) {
            return Ok(CallReply::NotRinging);
        }
        if !live {
            return Ok(CallReply::NoCall);
        }
    }
    match command {
        CallCommand::Answer => {
            p.call_answer()?;
            // Answered from a laptop: the phone's speaker, unless the
            // laptop carries the audio. Never active in time: unknown, so
            // the speaker is left alone.
            if answered_on_laptop(p, stop) == Some(false) {
                p.call_speaker(true)?;
            }
        }
        CallCommand::Decline => p.call_decline()?,
        CallCommand::End => p.call_end()?,
        CallCommand::Mute(on) => p.call_mute(*on)?,
        CallCommand::Dial { number } => p.call_dial(number)?,
        CallCommand::History { before } => {
            return Ok(CallReply::HistoryPage(
                p.call_history(*before, HISTORY_PAGE)?,
            ));
        }
        // Refused by `recv_calls`: they never cross the link.
        CallCommand::Tones(_) | CallCommand::AudioTo(_) => return Ok(CallReply::Failed),
    }
    Ok(CallReply::Done)
}
