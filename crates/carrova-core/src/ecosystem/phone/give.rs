//! The phone's side of notifications, SMS and calls: forwards what the
//! phone reports to every linked device allowing that feature, resends its
//! state when a device starts taking it or events were dropped, and serves
//! the devices' requests (reply, send a text, answer, ...). A device
//! taking notifications or texts hears when the phone lacks the access
//! they need, and again once it is back.
//!
//! One thread does all sending, so a device sees events in order. The
//! phone's events are watched only while some linked device takes a phone
//! feature. Never logs keys, titles, text, app names, numbers or names.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use carrova_crypto::identity::DeviceId;
use carrova_link::{
    ActionResult, CallCommand, CallIncoming, CallReply, Feature, FeatureState, HISTORY_PAGE,
    Incoming, LinkError, MAX_CONTACTS, MAX_SIMS, MESSAGE_PAGE, NotifyAction, NotifyMsg, PolicyDoc,
    SendOutcome, SmsIncoming, SmsPush, SmsReply, SmsRequest, THREAD_PAGE, allowed, check_sms_reply,
    fit_apps, fit_note, fit_snapshot, push_sms, recv_calls, recv_notify, recv_sms, send_call_state,
    send_notify,
};
use carrova_platform::phone::{CallState, Note, PermState, PhoneEvent, PhonePerm, PhoneWatch, Sim};
use carrova_platform::{Platform, PlatformError};
use carrova_transport::{Connection, Stream};

use super::apps::outgoing;
use super::{NotifyApps, SyncWindow};
use super::{calls, mirror};
use crate::cancel::Cancel;
use crate::ecosystem::supervisor::Shared;
use crate::error::Result;

/// The features a phone gives.
const PHONE: [Feature; 3] = [Feature::Notifications, Feature::Messages, Feature::Calls];

/// The watch's listeners that arm only while their permission is held,
/// with the feature each serves.
const LISTENERS: [(PhonePerm, Feature); 2] = [
    (PhonePerm::Calls, Feature::Calls),
    (PhonePerm::Sms, Feature::Messages),
];

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

/// Which pushed state a device takes (of SMS, only missing access: texts
/// are pulled).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Gives {
    notes: bool,
    calls: bool,
    sms: bool,
}

impl Gives {
    /// What this side gives a device on `local` whose policy is `remote`.
    pub(crate) fn of(local: &PolicyDoc, remote: Option<&PolicyDoc>) -> Self {
        let on = |f| remote.is_some_and(|r| allowed(local, r, f) == FeatureState::On);
        Self {
            notes: on(Feature::Notifications),
            calls: on(Feature::Calls),
            sms: on(Feature::Messages),
        }
    }

    /// What `self` gives that `before` did not.
    pub(crate) fn since(self, before: Self) -> Self {
        Self {
            notes: self.notes && !before.notes,
            calls: self.calls && !before.calls,
            sms: self.sms && !before.sms,
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
        unarmed: Vec::new(),
        tried: Vec::new(),
        sent: HashSet::new(),
        missing: Vec::new(),
    };
    while !shared.stop.is_cancelled() {
        giver.follow();
        giver.recheck();
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
    /// The watch's listeners whose permission was missing as it opened:
    /// rechecked each tick while a device takes their feature, and armed
    /// once granted (Android says nothing when a permission is granted).
    unarmed: Vec<PhonePerm>,
    /// The device features the last watch attempt was for: a failed one is
    /// tried again only once these change, never in a loop.
    tried: Vec<(DeviceId, Feature)>,
    /// Keys of the notes sent, so a removal of one never sent stays here.
    sent: HashSet<String>,
    /// Features whose access was reported missing: rechecked each tick
    /// (Android says nothing when a permission is granted).
    missing: Vec<Feature>,
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
            self.unarmed.clear();
            self.tried.clear();
            // Removals go unseen while unwatched: a later one must not
            // withdraw a note sent before.
            self.sent.clear();
            return;
        }
        if self.watch.is_some() {
            self.rearm(&want);
            return;
        }
        if want == self.tried {
            return;
        }
        self.tried = want;
        // Read before the watch arms its listeners: one granted in between
        // is armed again, never missed.
        let unarmed = LISTENERS
            .iter()
            .map(|&(perm, _)| perm)
            .filter(|&perm| !self.held(perm))
            .collect();
        let jobs = self.shared.give.clone();
        let on_event = Box::new(move |e| {
            let _ = jobs.send(Job::Event(e));
        });
        match self.platform().watch_phone_events(on_event) {
            Ok(watch) => {
                self.watch = Some(watch);
                self.unarmed = unarmed;
            }
            Err(e) => tracing::debug!(error = %e, "phone events not watched"),
        }
    }

    /// Arm each listener of the open watch whose permission is granted
    /// now, while a device in `want` takes its feature.
    fn rearm(&mut self, want: &[(DeviceId, Feature)]) {
        for perm in std::mem::take(&mut self.unarmed) {
            let feature = LISTENERS
                .iter()
                .find_map(|&(p, f)| (p == perm).then_some(f));
            let wanted = want.iter().any(|&(_, f)| Some(f) == feature);
            let armed = wanted
                && self.held(perm)
                && self.watch.as_ref().is_some_and(|w| match w.rearm(perm) {
                    Ok(()) => {
                        tracing::debug!(?perm, "phone listener armed");
                        true
                    }
                    Err(e) => {
                        tracing::debug!(?perm, error = %e, "phone listener not armed");
                        false
                    }
                });
            if !armed {
                self.unarmed.push(perm);
            }
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
                    sms: true,
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
        if !notes.is_empty() && self.has_access(&notes, Feature::Notifications) {
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
        if gives.sms {
            let sms = targets(Feature::Messages);
            if !sms.is_empty() {
                self.has_access(&sms, Feature::Messages);
            }
        }
    }

    /// Whether the phone holds the access `feature` needs; if not, tell
    /// `targets` (unreadable: assumed held, as before access was known).
    fn has_access(&mut self, targets: &Targets, feature: Feature) -> bool {
        if self.access(feature) {
            return true;
        }
        if !self.missing.contains(&feature) {
            self.missing.push(feature);
        }
        tell_access(targets, feature, false);
        false
    }

    /// Tell every device taking a feature whose access was missing that it
    /// is back; forget those no device takes now.
    fn recheck(&mut self) {
        let missing = std::mem::take(&mut self.missing);
        for feature in missing {
            let targets = self.shared.linked_with(feature);
            if targets.is_empty() {
                continue;
            }
            if self.access(feature) {
                tracing::debug!(?feature, "phone access granted");
                tell_access(&targets, feature, true);
            } else {
                self.missing.push(feature);
            }
        }
    }

    fn access(&self, feature: Feature) -> bool {
        self.held(perm(feature))
    }

    /// Whether the phone holds `perm` (unreadable: assumed held).
    fn held(&self, perm: PhonePerm) -> bool {
        self.platform()
            .phone_perm(perm)
            .map_or(true, |state| state == PermState::Granted)
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

/// The permission `feature` needs on the phone (notifications or texts).
fn perm(feature: Feature) -> PhonePerm {
    if feature == Feature::Notifications {
        PhonePerm::NotificationAccess
    } else {
        PhonePerm::Sms
    }
}

/// Tell `targets` whether the phone holds the access `feature` needs.
fn tell_access(targets: &Targets, feature: Feature, granted: bool) {
    if feature == Feature::Notifications {
        let msg = NotifyMsg::Access { granted };
        send(targets, |c| send_notify(c, &msg));
    } else {
        send(targets, |c| push_sms(c, &SmsPush::Access { granted }));
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
/// phone's message, carry out the device's action on a notification, or
/// give it the per-app list.
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
        Incoming::ListApps(responder) => {
            let platform = &**shared.platform();
            let apps = NotifyApps::open(platform)?;
            // Without notification access none show: the chosen apps only.
            let showing = platform.active_notes().unwrap_or_default();
            return Ok(responder.reply(&fit_apps(apps.entries(&showing)))?);
        }
    };
    let platform = &**shared.platform();
    let note = match &action {
        NotifyAction::Reply { key, .. }
        | NotifyAction::Dismiss { key }
        | NotifyAction::Open { key } => Some(key),
        NotifyAction::Remind => None,
    };
    // Only a note the device may see can be acted on.
    if let Some(key) = note
        && !forwarded(platform, key)
    {
        return Ok(responder.reply(&ActionResult::Gone)?);
    }
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
        NotifyAction::Remind => platform
            .remind_access(PhonePerm::NotificationAccess)
            .map(|()| ActionResult::Done),
    };
    let result = done.unwrap_or_else(|e| {
        tracing::debug!(?action, error = %e, "notification action failed");
        ActionResult::Failed
    });
    Ok(responder.reply(&result)?)
}

/// Whether the note `key` is showing and may leave the phone (fail closed
/// if either is unreadable).
fn forwarded(platform: &dyn Platform, key: &str) -> bool {
    let (Ok(apps), Ok(notes)) = (NotifyApps::open(platform), platform.active_notes()) else {
        return false;
    };
    notes
        .into_iter()
        .any(|n| n.key == key && outgoing(n, &apps).is_some())
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
    let since = since_ms(platform);
    let reply = match request {
        SmsRequest::Threads { before } => {
            SmsReply::ThreadsPage(platform.sms_threads(before, THREAD_PAGE, since)?)
        }
        SmsRequest::Thread { id, before } => {
            let mut page = platform.sms_thread(id, before, MESSAGE_PAGE)?;
            // Newest first: what is cut is older than the rest.
            page.retain(|m| since.is_none_or(|s| m.when_ms > s));
            SmsReply::ThreadPage(page)
        }
        SmsRequest::Send {
            to,
            text,
            client_id,
            sim,
        } => {
            // Kept on the device, not the link: a retry on a newer link after
            // a lost reply must not send again.
            let first = shared
                .peers()
                .iter_mut()
                .find(|p| p.id() == device)
                .filter(|p| p.link.as_ref().is_some_and(|l| l.generation == generation))
                .map(|p| p.sms_sent.begin(client_id));
            let outcome = match first {
                // The link closed: the answer could not arrive, so do not send.
                None => SendOutcome::Failed,
                Some(Some(again)) => again,
                Some(None) => {
                    let outcome = match sim_for(platform, sim) {
                        Some(sim) => platform.sms_send(&to, &text, sim).unwrap_or_else(|e| {
                            tracing::debug!(error = %e, "text not sent");
                            SendOutcome::Failed
                        }),
                        None => SendOutcome::Failed,
                    };
                    if let Some(p) = shared.peers().iter_mut().find(|p| p.id() == device) {
                        p.sms_sent.finish(client_id, outcome);
                    }
                    outcome
                }
            };
            SmsReply::SendResult { client_id, outcome }
        }
        SmsRequest::Sims => SmsReply::Sims(sims(platform)),
        SmsRequest::Remind => {
            let posted = platform.remind_access(PhonePerm::Sms);
            if let Err(e) = &posted {
                tracing::debug!(error = %e, "SMS reminder not shown");
            }
            SmsReply::Reminded {
                posted: posted.is_ok(),
            }
        }
    };
    check_sms_reply(&reply)?;
    Ok(responder.reply(&reply)?)
}

/// The phone's active SIMs, at most [`MAX_SIMS`]; none if unreadable
/// (texts and calls then use the default SIM, as on a single-SIM phone).
fn sims(platform: &dyn Platform) -> Vec<Sim> {
    let mut sims = platform.phone_sims().unwrap_or_else(|e| {
        tracing::debug!(error = %e, "SIMs unreadable");
        Vec::new()
    });
    sims.truncate(MAX_SIMS);
    sims
}

/// The SIM to send or call from for a device's choice `want`, which is
/// untrusted: `Some(None)` is the phone's default (no choice, or the SIMs
/// are unreadable here, as without `READ_PHONE_STATE`); `None` refuses a
/// SIM not active here rather than use one the user didn't pick.
fn sim_for(platform: &dyn Platform, want: Option<i32>) -> Option<Option<i32>> {
    let Some(id) = want else {
        return Some(None);
    };
    let sims = sims(platform);
    if sims.is_empty() {
        return Some(None);
    }
    let active = sims.iter().any(|s| s.id == id);
    if !active {
        tracing::debug!(sims = sims.len(), "chosen SIM not active here");
    }
    active.then_some(Some(id))
}

/// The oldest time SMS pages reach back to (the user's "Keep in sync");
/// `None`: no limit. An unreadable choice falls back to the narrowest.
fn since_ms(platform: &dyn Platform) -> Option<i64> {
    let window = SyncWindow::open(platform).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "messages sync window unreadable; keeping 30 days");
        SyncWindow::Days30
    });
    let now = i64::try_from(crate::time::now_unix()).unwrap_or(i64::MAX);
    window.since_ms(now.saturating_mul(1000))
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
    shared: &Arc<Shared>,
    device: DeviceId,
    generation: u64,
    stream: Stream,
) -> Result<()> {
    let (command, responder) = match recv_calls(stream)? {
        CallIncoming::State(state, ack) => {
            calls::take_call(shared, device, generation, state, ack);
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
            // The call is answered either way: a speaker failure is not
            // reported as a failed answer.
            if answered_on_laptop(p, stop) == Some(false)
                && let Err(e) = p.call_speaker(true)
            {
                tracing::debug!(error = %e, "speaker not turned on");
            }
        }
        CallCommand::Decline => p.call_decline()?,
        CallCommand::End => p.call_end()?,
        CallCommand::Mute(on) => p.call_mute(*on)?,
        CallCommand::Dial { number, sim } => match sim_for(p, *sim) {
            Some(sim) => p.call_dial(number, sim)?,
            None => return Ok(CallReply::Failed),
        },
        CallCommand::Sims => return Ok(CallReply::Sims(sims(p))),
        CallCommand::Contacts { query } => {
            // None without contacts permission: no suggestions, not a failure.
            let found = p.phone_contacts(query, MAX_CONTACTS).unwrap_or_else(|e| {
                tracing::debug!(error = %e, "contacts unreadable");
                Vec::new()
            });
            return Ok(CallReply::Contacts(
                found.into_iter().take(MAX_CONTACTS).collect(),
            ));
        }
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
