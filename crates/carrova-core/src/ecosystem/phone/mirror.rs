//! The laptop's side of notifications, SMS and calls: a mirror of what a
//! linked phone pushes, held in memory on its link only (never on disk),
//! and turned into events for the front-ends.
//!
//! A mirror lives and dies with its link: closing the link, forgetting
//! the device or turning a feature off (on either side) drops that data
//! and reports [`Event::PhoneCleared`]. Never logs keys, titles, text, app
//! names, numbers or names: counts and ids only.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use carrova_crypto::identity::DeviceId;
use carrova_link::{
    Feature, FeatureState, MAX_SNAPSHOT, NotifyMsg, Pending, PolicyDoc, SmsPush, SmsReply,
    SmsRequest, allowed, request_sms,
};
use carrova_platform::ScreenLock;
use carrova_platform::phone::{Cursor, Hidden, Note, SmsMessage};

use super::PhoneSettings;
use super::calls::{Call, CallView};
use crate::ecosystem::supervisor::Shared;
use crate::session::Event;

/// At most one pop-up ([`Event::PhoneNote`]) per app this often.
const POP_EVERY: Duration = Duration::from_secs(2);
/// A provisional text with no `Stored` this long after it arrived has its
/// thread refetched anyway.
const STORE_WAIT: Duration = Duration::from_secs(10);

/// The phone features a mirror holds.
const MIRRORED: [Feature; 3] = [Feature::Notifications, Feature::Messages, Feature::Calls];

/// What a linked phone showed this computer last, on one link.
#[derive(Default)]
pub(crate) struct Mirror {
    /// Notifications showing on the phone, newest first, one per key.
    notes: Vec<Note>,
    /// A snapshot filled `notes` (the first only fills, without pop-ups).
    seeded: bool,
    /// When each app last popped up.
    popped: HashMap<String, Instant>,
    /// The phone's call.
    pub(super) call: Call,
    /// Stored messages of the threads loaded here, newest first.
    threads: HashMap<i64, Vec<SmsMessage>>,
    /// Incoming texts the phone has not stored yet (`id` 0), and when.
    provisional: Vec<(SmsMessage, Instant)>,
}

impl std::fmt::Debug for Mirror {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mirror")
            .field("notes", &self.notes.len())
            .field("call", &self.call.state.is_some())
            .field("threads", &self.threads.len())
            .field("provisional", &self.provisional.len())
            .finish_non_exhaustive()
    }
}

/// What a notification message changed.
#[derive(Default)]
struct NoteChanges {
    /// New or changed notes to pop up.
    pop: Vec<Note>,
    /// Keys no longer showing.
    gone: Vec<String>,
}

impl Mirror {
    /// The features this mirror holds data for.
    pub(crate) fn held(&self) -> Vec<Feature> {
        MIRRORED.into_iter().filter(|&f| self.holds(f)).collect()
    }

    fn holds(&self, feature: Feature) -> bool {
        match feature {
            Feature::Notifications => !self.notes.is_empty(),
            Feature::Messages => !self.threads.is_empty() || !self.provisional.is_empty(),
            Feature::Calls => self.call.shown(),
            _ => false,
        }
    }

    /// Drop `feature`'s data; whether there was any.
    fn clear(&mut self, feature: Feature) -> bool {
        let held = self.holds(feature);
        match feature {
            Feature::Notifications => {
                self.notes.clear();
                self.popped.clear();
                self.seeded = false;
            }
            Feature::Messages => {
                self.threads.clear();
                self.provisional.clear();
            }
            Feature::Calls => self.call = Call::default(),
            _ => {}
        }
        held
    }

    /// Drop what is no longer on on both sides (`local` here, `remote` the
    /// phone's, if it arrived); returns the features cleared.
    pub(crate) fn follow_policy(
        &mut self,
        local: &PolicyDoc,
        remote: Option<&PolicyDoc>,
    ) -> Vec<Feature> {
        MIRRORED
            .into_iter()
            .filter(|&f| off(local, remote, f) && self.clear(f))
            .collect()
    }

    /// The notes showing, newest first.
    pub(crate) fn notes(&self) -> Vec<Note> {
        self.notes.clone()
    }

    fn notify(&mut self, msg: NotifyMsg, now: Instant) -> NoteChanges {
        match msg {
            NotifyMsg::Posted(note) => NoteChanges {
                pop: self.posted(note, now).into_iter().collect(),
                gone: Vec::new(),
            },
            NotifyMsg::Removed { key } => {
                let before = self.notes.len();
                self.notes.retain(|n| n.key != key);
                let gone = if self.notes.len() < before {
                    vec![key]
                } else {
                    Vec::new()
                };
                NoteChanges {
                    pop: Vec::new(),
                    gone,
                }
            }
            NotifyMsg::Snapshot(notes) => self.snapshot(notes, now),
        }
    }

    /// Keep `note` (replacing one of its key); it pops up unless it is
    /// already here unchanged (a note in a snapshot may also arrive
    /// posted) or its app popped up within [`POP_EVERY`].
    fn posted(&mut self, note: Note, now: Instant) -> Option<Note> {
        if self.notes.contains(&note) {
            return None;
        }
        self.notes.retain(|n| n.key != note.key);
        self.notes.insert(0, note.clone());
        self.notes.truncate(MAX_SNAPSHOT);
        self.pop(&note.app_id, now).then_some(note)
    }

    /// Replace every note with `notes` (newest first). The first snapshot
    /// holds what already showed on the phone, so nothing pops up; a later
    /// one (a resync) pops only notes not here before.
    fn snapshot(&mut self, mut notes: Vec<Note>, now: Instant) -> NoteChanges {
        notes.truncate(MAX_SNAPSHOT);
        // Duplicated keys: the first (newest) wins.
        let mut keys = Vec::with_capacity(notes.len());
        notes.retain(|n| {
            let fresh = !keys.contains(&n.key);
            keys.push(n.key.clone());
            fresh
        });
        let old = std::mem::replace(&mut self.notes, notes);
        let gone = old
            .iter()
            .filter(|o| !self.notes.iter().any(|n| n.key == o.key))
            .map(|o| o.key.clone())
            .collect();
        let seeded = std::mem::replace(&mut self.seeded, true);
        let fresh: Vec<Note> = self
            .notes
            .iter()
            .filter(|n| seeded && !old.iter().any(|o| o.key == n.key))
            .cloned()
            .collect();
        let pop = fresh
            .into_iter()
            .filter(|n| self.pop(&n.app_id, now))
            .collect();
        NoteChanges { pop, gone }
    }

    /// Whether `app` may pop up now (and if so, it did).
    fn pop(&mut self, app: &str, now: Instant) -> bool {
        self.popped
            .retain(|_, at| now.saturating_duration_since(*at) < POP_EVERY);
        if self.popped.contains_key(app) {
            return false;
        }
        self.popped.insert(app.to_owned(), now);
        true
    }

    /// Whether thread `id` is loaded here or waits for a stored text.
    fn wants(&self, id: i64) -> bool {
        self.threads.contains_key(&id) || self.provisional.iter().any(|(m, _)| m.thread_id == id)
    }

    /// Whether thread `id` still has a provisional text from `since`.
    fn waits_since(&self, id: i64, since: Instant) -> bool {
        self.provisional
            .iter()
            .any(|(m, at)| m.thread_id == id && *at <= since)
    }

    /// Take `page`, the newest of thread `id` after it was stored: each
    /// provisional text it holds goes (matched to one row by content, as
    /// the two sides' clocks differ); others stay for a later refetch.
    /// Returns the rows new to views, newest first: the matched ones (they
    /// replace those texts) and, if the thread is loaded, any not loaded.
    fn stored(&mut self, id: i64, page: &[SmsMessage]) -> Vec<SmsMessage> {
        // Rows a loaded thread lacks yet (none if it is not loaded).
        let unseen: Vec<SmsMessage> = self.threads.get(&id).map_or_else(Vec::new, |thread| {
            page.iter()
                .filter(|m| m.id != 0 && !thread.iter().any(|t| t.id == m.id))
                .cloned()
                .collect()
        });
        let mut free: Vec<&SmsMessage> = page.iter().filter(|m| m.id != 0).collect();
        let mut new: Vec<SmsMessage> = Vec::new();
        self.provisional.retain(|(p, _)| {
            if p.thread_id != id {
                return true;
            }
            let close = |m: &&SmsMessage| m.when_ms.abs_diff(p.when_ms);
            let row = free
                .iter()
                .enumerate()
                .filter(|(_, m)| is_stored_copy(p, m))
                .min_by_key(|(_, m)| close(m))
                .map(|(i, _)| i);
            match row {
                Some(i) => {
                    new.push(free.swap_remove(i).clone());
                    false
                }
                None => true,
            }
        });
        for m in unseen {
            if !new.iter().any(|n| n.id == m.id) {
                new.push(m);
            }
        }
        new.sort_by_key(|m| std::cmp::Reverse(cursor(m)));
        self.merge(id, page);
        new
    }

    /// Add a fetched page of thread `id` to what is loaded here.
    fn merge(&mut self, id: i64, page: &[SmsMessage]) {
        let thread = self.threads.entry(id).or_default();
        for m in page.iter().filter(|m| m.id != 0) {
            if !thread.iter().any(|t| t.id == m.id) {
                thread.push(m.clone());
            }
        }
        thread.sort_by_key(|m| std::cmp::Reverse(cursor(m)));
    }

    /// `page`, a reply about thread `id`, with the thread's provisional
    /// texts on top when it is the newest page.
    fn with_provisional(&self, id: i64, newest: bool, page: &[SmsMessage]) -> Vec<SmsMessage> {
        let mut shown: Vec<SmsMessage> = Vec::new();
        if newest {
            shown.extend(
                self.provisional
                    .iter()
                    .filter(|(m, _)| m.thread_id == id)
                    .map(|(m, _)| m.clone()),
            );
            shown.sort_by_key(|m| std::cmp::Reverse(m.when_ms));
        }
        shown.extend_from_slice(page);
        shown
    }
}

/// How far apart a provisional text and its stored row may be dated.
const STORED_SLACK_MS: u64 = 60_000;

/// Whether `row` is the stored copy of the provisional text `p`: same
/// body and direction, dated close by (compared here, never logged).
fn is_stored_copy(p: &SmsMessage, row: &SmsMessage) -> bool {
    row.text == p.text
        && row.outgoing == p.outgoing
        && row.when_ms.abs_diff(p.when_ms) <= STORED_SLACK_MS
}

/// Whether `feature` is off for a phone: here (`local`), or on both sides
/// once the phone's policy (`remote`) arrived.
fn off(local: &PolicyDoc, remote: Option<&PolicyDoc>, feature: Feature) -> bool {
    local.get(feature) == FeatureState::Off
        || remote.is_some_and(|r| allowed(local, r, feature) != FeatureState::On)
}

fn cursor(m: &SmsMessage) -> Cursor {
    Cursor {
        when_ms: m.when_ms,
        id: m.id,
    }
}

/// `note` as this computer may show it now: with no content while the
/// screen is (or may be) locked, unless the user shows it anyway.
fn shown(note: Note, lock: ScreenLock, settings: PhoneSettings) -> Note {
    if lock == ScreenLock::Unlocked || settings.show_when_locked {
        return note;
    }
    Note {
        app_name: String::new(),
        title: String::new(),
        text: String::new(),
        icon: None,
        hidden: Some(Hidden::Locked),
        ..note
    }
}

/// `notes` as this computer may show them now (see [`shown`]).
pub(crate) fn shown_now(shared: &Shared, notes: Vec<Note>) -> Vec<Note> {
    if notes.is_empty() {
        return notes;
    }
    // Never under the supervisor's lock: the OS may be slow to answer.
    let lock = shared.platform().screen_lock();
    let settings = shared.phone_settings();
    notes
        .into_iter()
        .map(|n| shown(n, lock, settings))
        .collect()
}

/// Emit [`Event::PhoneCleared`] for each of `features` (and
/// [`CallView::Left`] for calls).
pub(crate) fn cleared(shared: &Shared, device: DeviceId, features: Vec<Feature>) {
    for feature in features {
        shared.emit(Event::PhoneCleared { device, feature });
        if feature == Feature::Calls {
            shared.emit(Event::CallView(CallView::Left { device }));
        }
    }
}

/// Run `f` on the mirror of `device`'s link `generation`, if that link is
/// still live and `feature` is still allowed on both sides (either may
/// have turned it off since the lane was admitted).
pub(super) fn with_mirror<T>(
    shared: &Shared,
    device: DeviceId,
    generation: u64,
    feature: Feature,
    f: impl FnOnce(&mut Mirror) -> T,
) -> Option<T> {
    let mut peers = shared.peers();
    let peer = peers.iter_mut().find(|p| p.id() == device)?;
    let link = peer.link.as_mut().filter(|l| l.generation == generation)?;
    if off(&peer.local, link.remote.as_ref(), feature) {
        return None;
    }
    Some(f(&mut link.mirror))
}

/// Acknowledge a push once applied: only then does the phone send its
/// next one, so pushes apply (and emit) in the order sent.
pub(super) fn ack(pending: Pending) {
    if let Err(e) = pending.ack() {
        tracing::debug!(error = %e, "phone push not acknowledged");
    }
}

/// Apply a notification message from `device`'s link `generation`, then
/// acknowledge it.
pub(crate) fn take_notify(
    shared: &Shared,
    device: DeviceId,
    generation: u64,
    msg: NotifyMsg,
    pending: Pending,
) {
    let now = Instant::now();
    let changes = with_mirror(shared, device, generation, Feature::Notifications, |m| {
        m.notify(msg, now)
    });
    if let Some(changes) = changes {
        for key in changes.gone {
            shared.emit(Event::PhoneNoteGone {
                device,
                key: key.into(),
            });
        }
        if !changes.pop.is_empty() {
            // Read outside the supervisor's lock: the OS may be slow to answer.
            let lock = shared.platform().screen_lock();
            let settings = shared.phone_settings();
            // Away: the notes stay in the list, without pop-ups.
            if !settings.only_while_active || lock == ScreenLock::Unlocked {
                for note in changes.pop {
                    let note = shown(note, lock, settings);
                    shared.emit(Event::PhoneNote { device, note });
                }
            }
        }
    }
    ack(pending);
}

/// Apply an SMS push from `device`'s link `generation`, then acknowledge
/// it (a `Stored` first: its refetch asks the phone, which waits for the
/// acknowledgement before anything else).
pub(crate) fn take_sms(
    shared: &Arc<Shared>,
    device: DeviceId,
    generation: u64,
    push: SmsPush,
    pending: Pending,
) {
    const SMS: Feature = Feature::Messages;
    match push {
        SmsPush::Incoming(message) => {
            let now = Instant::now();
            let (id, provisional) = (message.thread_id, message.id == 0);
            let kept = with_mirror(shared, device, generation, SMS, |m| {
                if provisional {
                    m.provisional.push((message.clone(), now));
                } else {
                    m.merge(id, std::slice::from_ref(&message));
                }
            });
            if kept.is_some() {
                shared.emit(Event::PhoneSms { device, message });
                if provisional {
                    refetch_later(shared, device, generation, id, now);
                }
            }
            ack(pending);
        }
        SmsPush::Stored { thread_id } => {
            ack(pending);
            let wanted = with_mirror(shared, device, generation, SMS, |m| m.wants(thread_id));
            if wanted == Some(true) {
                refetch(shared, device, generation, thread_id);
            }
        }
        SmsPush::Resync => {
            let held = with_mirror(shared, device, generation, SMS, |m| m.clear(SMS));
            // Open views refetch on it, even with nothing held here.
            if held.is_some() {
                cleared(shared, device, vec![Feature::Messages]);
            }
            ack(pending);
        }
    }
}

/// After [`STORE_WAIT`], refetch thread `id` if its text from `since` is
/// still provisional (no `Stored` came).
fn refetch_later(shared: &Arc<Shared>, device: DeviceId, generation: u64, id: i64, since: Instant) {
    let waiting = Arc::clone(shared);
    let started = shared.spawn("carrova-sms-wait", move || {
        if waiting.stop.sleep(STORE_WAIT).is_err() {
            return;
        }
        let still = with_mirror(&waiting, device, generation, Feature::Messages, |m| {
            m.waits_since(id, since)
        });
        if still == Some(true) {
            refetch(&waiting, device, generation, id);
        }
    });
    if let Err(e) = started {
        tracing::debug!(error = %e, "stored text not awaited");
    }
}

/// Refetch the newest page of thread `id`: its provisional texts are
/// replaced by the stored ones ([`Event::PhoneSms`] for each new one).
fn refetch(shared: &Shared, device: DeviceId, generation: u64, id: i64) {
    let Some(conn) = shared.with_link(&device, generation, |l| Arc::clone(&l.conn)) else {
        return;
    };
    // Never under the supervisor's lock: this waits for the phone.
    let page = match request_sms(&conn, &SmsRequest::Thread { id, before: None }) {
        Ok(SmsReply::ThreadPage(page)) => page,
        Ok(_) => return,
        Err(e) => {
            tracing::debug!(thread = id, error = %e, "stored text not fetched");
            return;
        }
    };
    let new = with_mirror(shared, device, generation, Feature::Messages, |m| {
        m.stored(id, &page)
    });
    for message in new.into_iter().flatten() {
        shared.emit(Event::PhoneSms { device, message });
    }
}

/// Take `reply` to `request`, sent on `device`'s link `generation`: a
/// thread's page is kept, with its provisional texts on top when newest.
pub(crate) fn take_sms_reply(
    shared: &Shared,
    device: DeviceId,
    generation: u64,
    request: &SmsRequest,
    reply: SmsReply,
) -> SmsReply {
    let (&SmsRequest::Thread { id, before }, SmsReply::ThreadPage(page)) = (request, &reply) else {
        return reply;
    };
    let shown = with_mirror(shared, device, generation, Feature::Messages, |m| {
        m.merge(id, page);
        m.with_provisional(id, before.is_none(), page)
    });
    shown.map_or(reply, SmsReply::ThreadPage)
}
