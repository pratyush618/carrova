//! The laptop's mirror: two supervisors in one process, a scripted phone
//! linked to a laptop, and what the laptop holds and emits.

use carrova_core::ecosystem::{PeerState, PhoneSettings, Supervisor};
use carrova_core::{CoreError, Event};
use carrova_link::{
    ActionResult, AppEntry, Feature, FeatureState, NotifyAction, PolicyDoc, SendOutcome, SmsReply,
    SmsRequest,
};
use carrova_platform::ScreenLock;
use carrova_platform::phone::{
    CallState, Hidden, Note, PermState, PhoneEvent, PhonePerm, SmsMessage,
};

use crate::supervisor::device::{Device, fast, pair, start_both, wait_until};

const NOTES: Feature = Feature::Notifications;
const SMS: Feature = Feature::Messages;
const CALLS: Feature = Feature::Calls;

/// A ringing call, which the laptop shows (an idle one holds nothing).
fn ringing() -> CallState {
    CallState::Ringing {
        number: "5550100".into(),
        name: None,
    }
}

/// Clipboard and `on` on, the rest off.
fn doc(on: &[Feature]) -> PolicyDoc {
    let mut doc = PolicyDoc::computer_defaults();
    for &f in [Feature::Clipboard].iter().chain(on) {
        doc.features.insert(f, FeatureState::On);
    }
    doc
}

/// A phone and a laptop, both allowing `on`, linked with each other's
/// policy: `(phone, its supervisor, laptop, its supervisor)`. `setup`
/// scripts the phone first.
fn linked(on: &[Feature], setup: impl FnOnce(&Device)) -> (Device, Supervisor, Device, Supervisor) {
    let (laptop, phone, _) = pair(&doc(on));
    setup(&phone);
    let (sl, sp) = start_both(&laptop, &phone);
    wait_until("both hold the other's policy", || {
        let has = |s: &Supervisor| {
            s.status()
                .iter()
                .any(|p| p.state == PeerState::Linked && p.remote.is_some())
        };
        has(&sl) && has(&sp)
    });
    (phone, sp, laptop, sl)
}

fn note(key: &str, app_id: &str) -> Note {
    Note {
        key: key.into(),
        app_id: app_id.into(),
        app_name: "Chat".into(),
        title: "Ana".into(),
        text: "see you at 6".into(),
        when_ms: 1,
        can_reply: true,
        icon: Some(vec![1, 2, 3]),
        hidden: None,
    }
}

fn now_ms() -> i64 {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    i64::try_from(since.as_millis()).unwrap()
}

fn text(id: i64, thread_id: i64, when_ms: i64) -> SmsMessage {
    saying(id, thread_id, when_ms, "on my way")
}

fn saying(id: i64, thread_id: i64, when_ms: i64, body: &str) -> SmsMessage {
    SmsMessage {
        id,
        thread_id,
        outgoing: false,
        text: body.into(),
        when_ms,
    }
}

fn wait_watching(phone: &Device) {
    wait_until("the phone's events watched", || phone.platform.watching());
}

/// The laptop's mirror of `phone` holds `n` notes.
fn wait_notes(sl: &Supervisor, phone: &Device, n: usize) -> Vec<Note> {
    let mut notes = Vec::new();
    wait_until("the mirrored notes", || {
        notes = sl.notes(&phone.id).unwrap();
        notes.len() == n
    });
    notes
}

fn popped(laptop: &Device) -> Vec<Note> {
    laptop
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::PhoneNote { note, .. } => Some(note),
            _ => None,
        })
        .collect()
}

fn cleared(laptop: &Device, phone: &Device, feature: Feature) -> bool {
    laptop.count(|e| {
        *e == Event::PhoneCleared {
            device: phone.id,
            feature,
        }
    }) > 0
}

#[test]
fn feature_off_clears_mirror() {
    let (phone, sp, laptop, sl) = linked(&[NOTES, CALLS], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
        p.platform.phone().call = ringing();
    });
    wait_notes(&sl, &phone, 1);
    laptop.wait("the call state", |e| matches!(e, Event::PhoneCall { .. }));

    // Off here.
    sl.set_feature(&phone.id, NOTES, FeatureState::Off).unwrap();
    assert!(sl.notes(&phone.id).unwrap().is_empty());
    assert!(cleared(&laptop, &phone, NOTES));

    // Off on the phone.
    sp.set_feature(&laptop.id, CALLS, FeatureState::Off)
        .unwrap();
    wait_until("calls cleared", || cleared(&laptop, &phone, CALLS));
}

#[test]
fn forget_clears_all_phone_data() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES, CALLS], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
        p.platform.phone().call = ringing();
    });
    wait_notes(&sl, &phone, 1);
    laptop.wait("the call state", |e| matches!(e, Event::PhoneCall { .. }));

    laptop.forget(&phone.id);

    wait_until("everything cleared", || {
        cleared(&laptop, &phone, NOTES) && cleared(&laptop, &phone, CALLS)
    });
    assert!(matches!(sl.notes(&phone.id), Err(CoreError::UnknownDevice)));
}

#[test]
fn link_close_clears_mirror() {
    let (phone, sp, laptop, sl) = linked(&[NOTES], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
    });
    wait_notes(&sl, &phone, 1);

    sp.stop();

    wait_until("notes cleared", || cleared(&laptop, &phone, NOTES));
    assert!(sl.notes(&phone.id).unwrap().is_empty());
}

#[test]
fn burst_coalesces_per_app() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES], |_| {});
    laptop.platform.inner.set_screen_lock(ScreenLock::Unlocked);
    wait_watching(&phone);

    for i in 0..5 {
        let posted = note(&format!("a{i}"), "org.example.chat");
        phone.platform.script_event(PhoneEvent::NotePosted(posted));
    }
    let mail = note("b", "org.example.mail");
    phone.platform.script_event(PhoneEvent::NotePosted(mail));

    wait_notes(&sl, &phone, 6);
    // The last note is kept before it pops up: wait for its pop-up too.
    wait_until("both pop-ups", || popped(&laptop).len() >= 2);
    let apps: Vec<String> = popped(&laptop).into_iter().map(|n| n.app_id).collect();
    assert_eq!(apps, ["org.example.chat", "org.example.mail"]);
}

#[test]
fn note_in_snapshot_and_posted_pops_once() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
    });
    wait_watching(&phone);
    wait_notes(&sl, &phone, 1);

    // Posted while the link opened: it arrives again, unchanged.
    let again = note("k1", "org.example.chat");
    phone.platform.script_event(PhoneEvent::NotePosted(again));
    let other = note("k2", "org.example.mail");
    phone.platform.script_event(PhoneEvent::NotePosted(other));

    let notes = wait_notes(&sl, &phone, 2);
    assert_eq!(notes[1].key, "k1");
    laptop.wait("the pop-up", |e| matches!(e, Event::PhoneNote { .. }));
    // The link-open snapshot fills the mirror without pop-ups.
    let keys: Vec<String> = popped(&laptop).into_iter().map(|n| n.key).collect();
    assert_eq!(keys, ["k2"]);
}

#[test]
fn reconnect_does_not_pop_again() {
    let (phone, sp, laptop, sl) = linked(&[NOTES], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
    });
    wait_notes(&sl, &phone, 1);
    sp.stop();
    wait_until("notes cleared", || cleared(&laptop, &phone, NOTES));

    let sp = phone.start(fast());
    laptop.sees(&phone, &sp);
    phone.sees(&laptop, &sl);
    wait_notes(&sl, &phone, 1);

    assert!(popped(&laptop).is_empty());
}

#[test]
fn resync_pops_only_new_notes() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
    });
    wait_watching(&phone);
    wait_notes(&sl, &phone, 1);
    phone.platform.phone().notes = vec![
        note("k2", "org.example.mail"),
        note("k1", "org.example.chat"),
    ];

    phone
        .platform
        .script_event(PhoneEvent::Dropped { count: 1 });

    wait_notes(&sl, &phone, 2);
    laptop.wait("the pop-up", |e| matches!(e, Event::PhoneNote { .. }));
    let keys: Vec<String> = popped(&laptop).into_iter().map(|n| n.key).collect();
    assert_eq!(keys, ["k2"]);
}

#[test]
fn removed_note_is_gone() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
    });
    wait_watching(&phone);
    wait_notes(&sl, &phone, 1);

    let key = "k1".to_owned();
    phone
        .platform
        .script_event(PhoneEvent::NoteRemoved { key: key.clone() });

    let device = phone.id;
    laptop.wait("the note gone", |e| {
        *e == Event::PhoneNoteGone {
            device,
            key: key.clone().into(),
        }
    });
    assert!(sl.notes(&phone.id).unwrap().is_empty());
}

#[test]
fn locked_screen_hides_content() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES], |_| {});
    laptop.platform.inner.set_screen_lock(ScreenLock::Locked);
    wait_watching(&phone);

    let posted = note("k1", "org.example.chat");
    phone
        .platform
        .script_event(PhoneEvent::NotePosted(posted.clone()));

    let shown = laptop.wait("the pop-up", |e| matches!(e, Event::PhoneNote { .. }));
    let hidden = Note {
        app_name: String::new(),
        title: String::new(),
        text: String::new(),
        icon: None,
        hidden: Some(Hidden::Locked),
        ..posted.clone()
    };
    assert_eq!(
        shown,
        Event::PhoneNote {
            device: phone.id,
            note: hidden.clone()
        }
    );
    // The list hides it too; the mirror keeps it whole for once unlocked.
    assert_eq!(sl.notes(&phone.id).unwrap(), [hidden]);
    laptop.platform.inner.set_screen_lock(ScreenLock::Unlocked);
    assert_eq!(sl.notes(&phone.id).unwrap(), [posted]);
}

#[test]
fn show_when_locked_shows_content() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES], |_| {});
    laptop.platform.inner.set_screen_lock(ScreenLock::Locked);
    let settings = PhoneSettings {
        show_when_locked: true,
        ..PhoneSettings::default()
    };
    sl.set_phone_settings(settings).unwrap();
    wait_watching(&phone);

    let posted = note("k1", "org.example.chat");
    phone
        .platform
        .script_event(PhoneEvent::NotePosted(posted.clone()));

    laptop.wait("the pop-up", |e| matches!(e, Event::PhoneNote { .. }));
    assert_eq!(popped(&laptop), std::slice::from_ref(&posted));
    assert_eq!(sl.notes(&phone.id).unwrap(), [posted]);
}

#[test]
fn unknown_lock_fails_closed() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES], |_| {});
    laptop.platform.inner.set_screen_lock(ScreenLock::Unknown);
    wait_watching(&phone);

    let posted = note("k1", "org.example.chat");
    phone.platform.script_event(PhoneEvent::NotePosted(posted));

    laptop.wait("the pop-up", |e| matches!(e, Event::PhoneNote { .. }));
    let shown = popped(&laptop).remove(0);
    assert_eq!(shown.hidden, Some(Hidden::Locked));
    assert!(shown.title.is_empty() && shown.text.is_empty() && shown.app_name.is_empty());
    assert!(shown.icon.is_none());
    assert_eq!(sl.notes(&phone.id).unwrap(), [shown]);
}

#[test]
fn phone_settings_default_and_persist() {
    let (laptop, _phone, _) = pair(&doc(&[]));
    let sl = laptop.start(fast());
    assert_eq!(sl.phone_settings(), PhoneSettings::default());
    assert_eq!(
        PhoneSettings::default().ring_here,
        carrova_core::ecosystem::RingHere::WhileActive
    );

    let changed = PhoneSettings {
        ring_here: carrova_core::ecosystem::RingHere::Never,
        only_while_active: true,
        show_when_locked: true,
    };
    sl.set_phone_settings(changed).unwrap();
    sl.stop();

    assert_eq!(laptop.start(fast()).phone_settings(), changed);
}

#[test]
fn call_state_mirrored() {
    let (phone, _sp, laptop, _sl) = linked(&[CALLS], |_| {});
    wait_watching(&phone);
    let ringing = CallState::Ringing {
        number: "5550100".into(),
        name: None,
    };

    phone
        .platform
        .script_event(PhoneEvent::Call(ringing.clone()));

    let device = phone.id;
    laptop.wait("the ringing", |e| {
        *e == Event::PhoneCall {
            device,
            state: ringing.clone(),
        }
    });
}

fn send(client_id: u64) -> SmsRequest {
    SmsRequest::Send {
        to: "5550100".into(),
        text: "on my way".into(),
        client_id,
    }
}

fn sends(phone: &Device) -> usize {
    let actions = phone.platform.actions();
    actions.iter().filter(|a| *a == "sms_send").count()
}

#[test]
fn send_twice_same_client_id_sends_once() {
    let (phone, _sp, _laptop, sl) = linked(&[SMS], |_| {});

    let first = sl.sms(&phone.id, &send(7)).unwrap();
    let second = sl.sms(&phone.id, &send(7)).unwrap();

    let sent = SmsReply::SendResult {
        client_id: 7,
        outcome: SendOutcome::Sent,
    };
    assert!(first == sent && second == sent, "{first:?} {second:?}");
    assert_eq!(sends(&phone), 1);
}

#[test]
fn send_while_away_refused() {
    let (laptop, phone, _) = pair(&doc(&[SMS]));
    let sl = laptop.start(fast());

    let refused = sl.sms(&phone.id, &send(7));

    assert!(matches!(refused, Err(CoreError::NotLinked)), "{refused:?}");
    assert_eq!(sends(&phone), 0);
}

/// The laptop's newest page of `thread`.
fn newest(sl: &Supervisor, phone: &Device, thread: i64) -> Vec<SmsMessage> {
    match sl.sms(
        &phone.id,
        &SmsRequest::Thread {
            id: thread,
            before: None,
        },
    ) {
        Ok(SmsReply::ThreadPage(page)) => page,
        other => panic!("not a page: {other:?}"),
    }
}

fn texts(laptop: &Device) -> Vec<SmsMessage> {
    laptop
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::PhoneSms { message, .. } => Some(message),
            _ => None,
        })
        .collect()
}

#[test]
fn provisional_sms_replaced_after_stored() {
    let at = now_ms();
    let old = text(1, 5, at - 60_000);
    let (phone, _sp, laptop, sl) = linked(&[SMS], |p| {
        p.platform.phone().messages = vec![old.clone()];
    });
    wait_watching(&phone);
    assert_eq!(newest(&sl, &phone, 5), std::slice::from_ref(&old));

    let provisional = text(0, 5, at);
    phone
        .platform
        .script_event(PhoneEvent::SmsReceived(provisional.clone()));
    wait_until("the provisional text", || {
        texts(&laptop) == [provisional.clone()]
    });
    assert_eq!(newest(&sl, &phone, 5), [provisional.clone(), old.clone()]);

    let stored = text(2, 5, at);
    phone.platform.phone().messages.push(stored.clone());
    phone
        .platform
        .script_event(PhoneEvent::SmsStored { thread_id: 5 });

    wait_until("the stored text", || {
        texts(&laptop) == [provisional.clone(), stored.clone()]
    });
    assert_eq!(newest(&sl, &phone, 5), [stored, old]);
}

#[test]
fn provisional_sms_refetched_without_stored() {
    let at = now_ms();
    let (phone, _sp, laptop, sl) = linked(&[SMS], |_| {});
    wait_watching(&phone);

    let provisional = text(0, 9, at);
    phone
        .platform
        .script_event(PhoneEvent::SmsReceived(provisional.clone()));
    wait_until("the provisional text", || texts(&laptop).len() == 1);
    let stored = text(3, 9, at);
    phone.platform.phone().messages.push(stored.clone());

    // No `Stored` comes: refetched after 10 s anyway.
    wait_until("the stored text", || {
        texts(&laptop) == [provisional.clone(), stored.clone()]
    });
    assert_eq!(newest(&sl, &phone, 9), [stored]);
}

#[test]
fn resync_drops_texts_and_tells_views() {
    let (phone, _sp, laptop, sl) = linked(&[SMS], |_| {});
    wait_watching(&phone);
    let provisional = text(0, 5, now_ms());
    phone
        .platform
        .script_event(PhoneEvent::SmsReceived(provisional.clone()));
    wait_until("the provisional text", || texts(&laptop).len() == 1);

    phone
        .platform
        .script_event(PhoneEvent::Dropped { count: 3 });

    wait_until("messages cleared", || cleared(&laptop, &phone, SMS));
    assert!(newest(&sl, &phone, 5).is_empty());
}

#[test]
fn posted_then_removed_leaves_no_note() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES], |_| {});
    // A pop-up is slow to show: the removal must still come after it.
    laptop.platform.slow_screen_lock();
    wait_watching(&phone);

    let key = "k1".to_owned();
    let posted = note(&key, "org.example.chat");
    phone.platform.script_event(PhoneEvent::NotePosted(posted));
    phone
        .platform
        .script_event(PhoneEvent::NoteRemoved { key: key.clone() });

    let device = phone.id;
    let gone = Event::PhoneNoteGone {
        device,
        key: key.into(),
    };
    laptop.wait("the removal", |e| *e == gone);
    let events = laptop.events();
    let at = |f: &dyn Fn(&Event) -> bool| events.iter().position(f).unwrap();
    assert!(at(&|e| matches!(e, Event::PhoneNote { .. })) < at(&|e| *e == gone));
    assert!(sl.notes(&phone.id).unwrap().is_empty());
}

/// Script `msg` arriving on the phone (provisional, `id` 0).
fn receive(phone: &Device, msg: &SmsMessage) {
    let received = PhoneEvent::SmsReceived(msg.clone());
    phone.platform.script_event(received);
}

/// Store `rows` on the phone and say so for thread `id`.
fn store(phone: &Device, id: i64, rows: &[SmsMessage]) {
    phone.platform.phone().messages.extend_from_slice(rows);
    phone
        .platform
        .script_event(PhoneEvent::SmsStored { thread_id: id });
}

#[test]
fn unstored_provisional_text_kept_after_a_refetch() {
    let at = now_ms();
    let (phone, _sp, laptop, sl) = linked(&[SMS], |_| {});
    wait_watching(&phone);
    let first = saying(0, 5, at, "on my way");
    let second = saying(0, 5, at + 1000, "running late");
    receive(&phone, &first);
    receive(&phone, &second);
    wait_until("both provisional texts", || texts(&laptop).len() == 2);

    // Only the first is stored when its `Stored` comes.
    let stored = saying(2, 5, at, "on my way");
    store(&phone, 5, std::slice::from_ref(&stored));
    wait_until("the stored text", || texts(&laptop).len() == 3);

    assert_eq!(texts(&laptop)[2], stored);
    assert_eq!(newest(&sl, &phone, 5), [second, stored]);
}

#[test]
fn stored_a_little_earlier_replaces_its_provisional_text() {
    // The default SMS app dates the row a few ms before Carrova's receiver.
    let at = now_ms();
    let (phone, _sp, laptop, sl) = linked(&[SMS], |_| {});
    wait_watching(&phone);
    receive(&phone, &text(0, 5, at));
    wait_until("the provisional text", || texts(&laptop).len() == 1);

    let stored = text(2, 5, at - 50);
    store(&phone, 5, std::slice::from_ref(&stored));
    wait_until("the stored text", || texts(&laptop).len() == 2);

    assert_eq!(newest(&sl, &phone, 5), [stored]);
}

#[test]
fn provisional_text_kept_until_its_row_is_stored() {
    let at = now_ms();
    let (phone, _sp, laptop, sl) = linked(&[SMS], |_| {});
    wait_watching(&phone);
    let provisional = text(0, 5, at);
    receive(&phone, &provisional);
    wait_until("the provisional text", || texts(&laptop).len() == 1);

    // Stored elsewhere first: the refetched page does not hold it.
    store(&phone, 5, &[]);
    wait_until("the refetch", || {
        phone.platform.actions().iter().any(|a| a == "sms_thread 5")
    });
    assert_eq!(newest(&sl, &phone, 5), [provisional]);

    let stored = text(2, 5, at - 50);
    store(&phone, 5, std::slice::from_ref(&stored));
    wait_until("the stored text", || texts(&laptop).len() == 2);
    assert_eq!(newest(&sl, &phone, 5), [stored]);
}

#[test]
fn identical_provisional_texts_match_rows_one_to_one() {
    let at = now_ms();
    let (phone, _sp, laptop, sl) = linked(&[SMS], |_| {});
    wait_watching(&phone);
    receive(&phone, &saying(0, 5, at, "ok"));
    receive(&phone, &saying(0, 5, at + 1000, "ok"));
    wait_until("both provisional texts", || texts(&laptop).len() == 2);

    let rows = [saying(3, 5, at + 950, "ok"), saying(2, 5, at - 50, "ok")];
    store(&phone, 5, &rows);
    wait_until("both stored texts", || texts(&laptop).len() == 4);

    assert_eq!(newest(&sl, &phone, 5), rows);
}

#[test]
fn pop_ups_held_while_away() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES], |_| {});
    laptop.platform.inner.set_screen_lock(ScreenLock::Locked);
    let settings = PhoneSettings {
        only_while_active: true,
        ..PhoneSettings::default()
    };
    sl.set_phone_settings(settings).unwrap();
    wait_watching(&phone);

    let reads = laptop.platform.lock_reads();
    let held = note("k1", "org.example.chat");
    phone.platform.script_event(PhoneEvent::NotePosted(held));
    wait_notes(&sl, &phone, 1);
    // Unlocked only once the held note's push read the lock.
    wait_until("the lock read", || laptop.platform.lock_reads() > reads);
    laptop.platform.inner.set_screen_lock(ScreenLock::Unlocked);
    let shown = note("k2", "org.example.mail");
    phone
        .platform
        .script_event(PhoneEvent::NotePosted(shown.clone()));

    laptop.wait("the pop-up", |e| matches!(e, Event::PhoneNote { .. }));
    assert_eq!(popped(&laptop), [shown]);
    assert_eq!(sl.notes(&phone.id).unwrap().len(), 2);
}

/// The phone features `phone` lacks access for, as the laptop's status says.
fn missing(sl: &Supervisor, phone: &Device) -> Vec<Feature> {
    let peer = sl.status().into_iter().find(|p| p.id == phone.id);
    peer.map(|p| p.missing).unwrap_or_default()
}

#[test]
fn missing_access_shows_on_the_device_until_granted() {
    let (phone, _sp, laptop, sl) = linked(&[NOTES, SMS], |p| {
        p.platform.phone().perms = vec![
            (PhonePerm::NotificationAccess, PermState::Denied),
            (PhonePerm::Sms, PermState::Restricted),
        ];
    });
    wait_until("both reported missing", || {
        let m = missing(&sl, &phone);
        m.len() == 2 && m.contains(&NOTES) && m.contains(&SMS)
    });
    let told = |feature, granted| {
        laptop.count(|e| {
            *e == Event::PhoneAccess {
                device: phone.id,
                feature,
                granted,
            }
        })
    };
    assert_eq!((told(NOTES, false), told(SMS, false)), (1, 1));

    phone.platform.phone().perms.clear();
    wait_until("access back", || missing(&sl, &phone).is_empty());
    laptop.wait("told SMS is back", |e| {
        *e == Event::PhoneAccess {
            device: phone.id,
            feature: SMS,
            granted: true,
        }
    });
}

#[test]
fn resync_keeps_missing_sms_access() {
    let (phone, _sp, laptop, sl) = linked(&[SMS], |p| {
        p.platform.phone().perms = vec![(PhonePerm::Sms, PermState::Denied)];
    });
    wait_until("reported missing", || missing(&sl, &phone) == [SMS]);
    wait_watching(&phone);

    phone
        .platform
        .script_event(PhoneEvent::Dropped { count: 3 });

    wait_until("messages resynced", || cleared(&laptop, &phone, SMS));
    assert_eq!(missing(&sl, &phone), [SMS]);
}

#[test]
fn feature_off_forgets_missing_access() {
    let (phone, _sp, _laptop, sl) = linked(&[NOTES], |p| {
        p.platform.phone().perms = vec![(PhonePerm::NotificationAccess, PermState::Denied)];
    });
    wait_until("reported missing", || missing(&sl, &phone) == [NOTES]);

    sl.set_feature(&phone.id, NOTES, FeatureState::Off).unwrap();
    assert!(missing(&sl, &phone).is_empty());
}

#[test]
fn app_list_and_reminders_through_the_handle() {
    let (phone, _sp, _laptop, sl) = linked(&[NOTES, SMS], |p| {
        p.platform.phone().notes = vec![note("k1", "org.example.chat")];
    });

    let chat = AppEntry {
        app_id: "org.example.chat".into(),
        app_name: "Chat".into(),
        on: true,
    };
    assert_eq!(sl.phone_apps(&phone.id).unwrap(), [chat]);
    assert_eq!(
        sl.note_action(&phone.id, &NotifyAction::Remind).unwrap(),
        ActionResult::Done
    );
    assert_eq!(
        sl.sms(&phone.id, &SmsRequest::Remind).unwrap(),
        SmsReply::Reminded { posted: true }
    );
}
