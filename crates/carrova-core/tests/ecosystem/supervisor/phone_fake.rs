//! The [`Recording`] fake's scripted phone behaves as later tests rely on.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use carrova_platform::Platform;
use carrova_platform::fake::FakePlatform;
use carrova_platform::phone::{
    CallKind, CallRecord, CallState, Cursor, Note, PhoneEvent, PhoneWatch, SendOutcome, SmsMessage,
};

use super::recording::Recording;

fn recording() -> Recording {
    Recording::new(FakePlatform::with_home(std::env::temp_dir()))
}

fn note(key: &str) -> Note {
    Note {
        key: key.into(),
        app_id: "app".into(),
        app_name: "App".into(),
        title: "secret title".into(),
        text: "secret text".into(),
        when_ms: 1,
        can_reply: true,
        icon: None,
        hidden: None,
        sms_app: false,
    }
}

#[test]
fn events_buffer_until_watched_and_stop_on_drop() {
    let p = recording();
    let seen = Arc::new(Mutex::new(Vec::new()));
    p.script_event(PhoneEvent::Dropped { count: 1 });
    let sink = Arc::clone(&seen);
    let watch = p
        .watch_phone_events(Box::new(move |e| sink.lock().unwrap().push(e)))
        .unwrap();
    p.script_event(PhoneEvent::Dropped { count: 2 });
    // The buffered event is replayed on another thread: wait for both.
    let deadline = Instant::now() + Duration::from_secs(5);
    while seen.lock().unwrap().len() < 2 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    drop(watch);
    p.script_event(PhoneEvent::Dropped { count: 3 });
    assert_eq!(
        *seen.lock().unwrap(),
        [
            PhoneEvent::Dropped { count: 1 },
            PhoneEvent::Dropped { count: 2 }
        ]
    );
}

fn wait_len(seen: &Mutex<Vec<PhoneEvent>>, len: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while seen.lock().unwrap().len() < len && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn replacing_a_watch_moves_events_and_survives_the_old_drop() {
    let p = recording();
    let (a, b) = (
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(Vec::new())),
    );
    let sink = Arc::clone(&a);
    let old = p
        .watch_phone_events(Box::new(move |e| sink.lock().unwrap().push(e)))
        .unwrap();
    p.script_event(PhoneEvent::Dropped { count: 1 });
    wait_len(&a, 1);
    let sink = Arc::clone(&b);
    let _new = p
        .watch_phone_events(Box::new(move |e| sink.lock().unwrap().push(e)))
        .unwrap();
    p.script_event(PhoneEvent::Dropped { count: 2 });
    wait_len(&b, 1);
    drop(old);
    p.script_event(PhoneEvent::Dropped { count: 3 });
    wait_len(&b, 2);
    assert_eq!(*a.lock().unwrap(), [PhoneEvent::Dropped { count: 1 }]);
    assert_eq!(
        *b.lock().unwrap(),
        [
            PhoneEvent::Dropped { count: 2 },
            PhoneEvent::Dropped { count: 3 }
        ]
    );
}

#[test]
fn scripted_answers_and_content_free_actions() {
    let p = recording();
    p.phone().notes.push(note("k1"));
    assert!(p.note_reply("k1", "secret reply").unwrap());
    assert!(!p.note_reply("gone", "secret reply").unwrap());
    p.note_dismiss("k1").unwrap();
    assert!(p.active_notes().unwrap().is_empty());
    assert_eq!(p.sms_send("555", "secret").unwrap(), SendOutcome::Sent);
    p.phone().send_outcome = SendOutcome::NoSignal;
    assert_eq!(p.sms_send("555", "secret").unwrap(), SendOutcome::NoSignal);
    let actions = p.actions();
    assert_eq!(actions[0], "note_reply k1");
    assert!(
        actions
            .iter()
            .all(|a| !a.contains("secret") && !a.contains("555"))
    );
}

/// Pages through `fetch` (limit 50) until a short page; every id once each.
fn page_all(fetch: impl Fn(Option<Cursor>) -> Vec<Cursor>) -> Vec<i64> {
    let mut ids = Vec::new();
    let mut before = None;
    loop {
        let page = fetch(before);
        ids.extend(page.iter().map(|c| c.id));
        if page.len() < 50 {
            break;
        }
        before = page.last().copied();
    }
    ids.sort_unstable();
    ids
}

#[test]
fn paging_with_equal_timestamps_is_complete() {
    let p = recording();
    for id in 0..51 {
        p.phone().messages.push(SmsMessage {
            id,
            thread_id: 7,
            outgoing: false,
            text: String::new(),
            when_ms: 1_000,
        });
        p.phone().history.push(CallRecord {
            id,
            number: String::new(),
            name: None,
            when_ms: 1_000,
            duration_s: 0,
            kind: CallKind::Incoming,
        });
    }
    let all: Vec<i64> = (0..51).collect();
    let sms = page_all(|before| {
        let page = p.sms_thread(7, before, 50).unwrap();
        page.iter()
            .map(|m| Cursor {
                when_ms: m.when_ms,
                id: m.id,
            })
            .collect()
    });
    assert_eq!(sms, all);
    let calls = page_all(|before| {
        let page = p.call_history(before, 50).unwrap();
        page.iter()
            .map(|c| Cursor {
                when_ms: c.when_ms,
                id: c.id,
            })
            .collect()
    });
    assert_eq!(calls, all);
}

#[test]
fn call_state_is_scripted() {
    let p = recording();
    assert_eq!(p.call_state().unwrap(), CallState::Idle);
    p.phone().call = CallState::Ended { missed: true };
    assert_eq!(p.call_state().unwrap(), CallState::Ended { missed: true });
}

#[test]
fn replay_runs_off_the_registering_thread() {
    // A supervisor holding its own lock across watch_phone_events, whose
    // callback takes that lock, must not deadlock on buffered events.
    let p = recording();
    p.script_event(PhoneEvent::Dropped { count: 1 });
    let state = Arc::new(Mutex::new(0));
    let held = state.lock().unwrap();
    let cb_state = Arc::clone(&state);
    let _watch = p
        .watch_phone_events(Box::new(move |_| *cb_state.lock().unwrap() += 1))
        .unwrap();
    drop(held);
    let deadline = Instant::now() + Duration::from_secs(5);
    while *state.lock().unwrap() == 0 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(*state.lock().unwrap(), 1);
}

#[test]
fn live_events_run_off_the_scripting_thread() {
    // The scripting thread holds a lock the callback takes: delivery on
    // that thread would deadlock. Scripted from a spawned thread so a
    // regression fails on the timeout instead of hanging the run.
    let p = Arc::new(recording());
    let state = Arc::new(Mutex::new(()));
    let (done, finished) = mpsc::channel();
    let cb_state = Arc::clone(&state);
    let watch = p
        .watch_phone_events(Box::new(move |_| {
            drop(cb_state.lock().unwrap());
            let _ = done.send(());
        }))
        .unwrap();
    // Leaked: on a regression, dropping it would wait on the stuck fake.
    std::mem::forget(watch);
    // A first event proves the watch live, so the next is a live delivery.
    p.script_event(PhoneEvent::Dropped { count: 1 });
    finished
        .recv_timeout(Duration::from_secs(5))
        .expect("watch is live");
    let (script, held) = (Arc::clone(&p), Arc::clone(&state));
    thread::spawn(move || {
        let _held = held.lock().unwrap();
        script.script_event(PhoneEvent::Dropped { count: 2 });
    });
    finished
        .recv_timeout(Duration::from_secs(5))
        .expect("callback ran after the scripting thread let go");
}

#[test]
fn a_callback_may_drop_its_own_watch() {
    let p = Arc::new(recording());
    let slot: Arc<Mutex<Option<PhoneWatch>>> = Arc::default();
    let (done, finished) = mpsc::channel();
    let cb_slot = Arc::clone(&slot);
    let watch = p
        .watch_phone_events(Box::new(move |_| {
            let own = cb_slot.lock().unwrap().take();
            drop(own);
            let _ = done.send(());
        }))
        .unwrap();
    *slot.lock().unwrap() = Some(watch);
    let script = Arc::clone(&p);
    thread::spawn(move || script.script_event(PhoneEvent::Dropped { count: 1 }));
    finished
        .recv_timeout(Duration::from_secs(5))
        .expect("dropping its own watch did not deadlock");
    assert!(slot.lock().unwrap().is_none());
}

/// Stops a watch (via `stop`, on another thread) while its callback runs:
/// the stop must not return before that callback ends, and the event
/// queued behind it must never start.
fn stop_waits_for_the_running_callback(stop: fn(&Recording, PhoneWatch) -> Option<PhoneWatch>) {
    let p = Arc::new(recording());
    let (entered_tx, entered) = mpsc::channel();
    let (release, released) = mpsc::channel::<()>();
    let running = Arc::new(AtomicBool::new(false));
    let cb_running = Arc::clone(&running);
    let watch = p
        .watch_phone_events(Box::new(move |_| {
            cb_running.store(true, Ordering::SeqCst);
            let _ = entered_tx.send(());
            let _ = released.recv_timeout(Duration::from_secs(5));
            cb_running.store(false, Ordering::SeqCst);
        }))
        .unwrap();
    p.script_event(PhoneEvent::Dropped { count: 1 });
    entered
        .recv_timeout(Duration::from_secs(5))
        .expect("callback is running");
    p.script_event(PhoneEvent::Dropped { count: 2 });
    let (stopped_tx, stopped) = mpsc::channel();
    let (sp, sr) = (Arc::clone(&p), Arc::clone(&running));
    thread::spawn(move || {
        let kept = stop(&sp, watch);
        let _ = stopped_tx.send(sr.load(Ordering::SeqCst));
        drop(kept);
    });
    assert!(
        stopped.recv_timeout(Duration::from_millis(200)).is_err(),
        "stop returned while its callback was running"
    );
    release.send(()).unwrap();
    let still_running = stopped
        .recv_timeout(Duration::from_secs(5))
        .expect("stop returned once the callback ended");
    assert!(!still_running);
    assert!(
        entered.recv_timeout(Duration::from_millis(200)).is_err(),
        "a callback started after the stop returned"
    );
}

#[test]
fn dropping_a_watch_waits_for_its_running_callback() {
    stop_waits_for_the_running_callback(|_, watch| {
        drop(watch);
        None
    });
}

#[test]
fn replacing_a_watch_waits_for_its_running_callback() {
    stop_waits_for_the_running_callback(|p, old| {
        let new = p.watch_phone_events(Box::new(|_| {})).unwrap();
        drop(old);
        Some(new)
    });
}
