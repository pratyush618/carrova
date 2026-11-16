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
use crate::ecosystem::supervisor::Shared;
use crate::error::Result;

/// The features a phone gives.
const PHONE: [Feature; 3] = [Feature::Notifications, Feature::Messages, Feature::Calls];

/// How far back SMS pages go ("Keep in sync"), in ms.
// shortcut: fixed at the 30-day default; read the user's choice once the
// phone's Messages screen stores one.
const SYNC_WINDOW_MS: i64 = 30 * 24 * 60 * 60 * 1000;

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
            Ok(Job::Opened { device, gives }) => giver.resend(Some(device), gives),
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
                let Some(note) = self.filtered(vec![note]).pop() else {
                    return;
                };
                self.sent.insert(note.key.clone());
                let msg = NotifyMsg::Posted(fit_note(note));
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

/// Serve a `Notify` lane: carry out the device's action on a notification.
pub(crate) fn serve_notify(platform: &dyn Platform, stream: Stream) -> Result<()> {
    // A message from a phone to this side: the mirror is not built yet.
    let Incoming::Action(action, responder) = recv_notify(stream)? else {
        return Ok(());
    };
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
    shared: &Shared,
    device: DeviceId,
    generation: u64,
    stream: Stream,
) -> Result<()> {
    // A push from a phone to this side: the mirror is not built yet.
    let SmsIncoming::Request(request, responder) = recv_sms(stream)? else {
        return Ok(());
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

/// Serve a `Calls` lane: carry out the device's call command.
pub(crate) fn serve_calls(platform: &dyn Platform, stream: Stream) -> Result<()> {
    // A state from a phone to this side: the calls controller is not built yet.
    let CallIncoming::Command(command, responder) = recv_calls(stream)? else {
        return Ok(());
    };
    let reply = call(platform, &command).unwrap_or_else(|e| {
        tracing::debug!(?command, error = %e, "call command failed");
        CallReply::Failed
    });
    Ok(responder.reply(&reply)?)
}

fn call(p: &dyn Platform, command: &CallCommand) -> std::result::Result<CallReply, PlatformError> {
    match command {
        CallCommand::Answer => {
            p.call_answer()?;
            // Answered from a laptop: the phone's speaker, unless the
            // laptop carries the audio.
            let on_laptop = matches!(
                p.call_state(),
                Ok(CallState::Active {
                    on_laptop_audio: true,
                    ..
                })
            );
            if !on_laptop {
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
