//! The phone through `Events.kt`, `Notify.kt`, `Sms.kt`, `Calls.kt` and
//! `Perms.kt`: notifications, SMS, calls and their permissions. Lists come
//! back as JSON (parsed in [`events`]); events are pulled by one thread
//! blocked in `Events.take()` until the watch guard closes the queue.

use std::cmp::Reverse;

use ::jni::JNIEnv;
use ::jni::objects::{JString, JValue, JValueOwned};

use super::{events, files, jni};
use crate::PlatformError;
use crate::phone::{
    CallRecord, CallState, Contact, Cursor, Note, PermState, PhoneEvent, PhonePerm, PhoneWatch,
    SendOutcome, Sim, SmsMessage, SmsThread,
};

const STRING: &str = "Ljava/lang/String;";

/// An argument of a glue call, after the context.
#[derive(Clone, Copy)]
enum Arg<'a> {
    Str(&'a str),
    Long(i64),
    Int(i32),
    Bool(bool),
}

/// `class.method(context, args…)` returning JNI type `ret`, read by `read`.
fn glue<T>(
    call: &'static str,
    class: &str,
    method: &str,
    args: &[Arg<'_>],
    ret: &str,
    read: impl FnOnce(&mut JNIEnv<'_>, JValueOwned<'_>) -> ::jni::errors::Result<T>,
) -> Result<T, PlatformError> {
    jni::call(call, |env, context| {
        let mut sig = String::from("(Landroid/content/Context;");
        let mut owned = Vec::with_capacity(args.len());
        for arg in args {
            let (code, value) = match *arg {
                Arg::Str(s) => (STRING, JValueOwned::Object(env.new_string(s)?.into())),
                Arg::Long(v) => ("J", JValueOwned::Long(v)),
                Arg::Int(v) => ("I", JValueOwned::Int(v)),
                Arg::Bool(v) => ("Z", JValueOwned::Bool(v.into())),
            };
            sig.push_str(code);
            owned.push(value);
        }
        sig.push(')');
        sig.push_str(ret);
        let mut values = vec![JValue::from(context)];
        values.extend(owned.iter().map(JValueOwned::borrow));
        let value = jni::call_static(env, context, class, method, &sig, &values)?;
        read(env, value)
    })
}

fn void(
    call: &'static str,
    class: &str,
    method: &str,
    args: &[Arg<'_>],
) -> Result<(), PlatformError> {
    glue(call, class, method, args, "V", |_, v| v.v())
}

fn text(
    call: &'static str,
    class: &str,
    method: &str,
    args: &[Arg<'_>],
) -> Result<String, PlatformError> {
    glue(call, class, method, args, STRING, files::string)
}

/// An exclusive page bound as the glue takes it: rows before `(date, id)`;
/// none is the end of time.
fn page_bound(cursor: Option<Cursor>) -> [Arg<'static>; 2] {
    let cursor = cursor.unwrap_or(Cursor {
        when_ms: i64::MAX,
        id: i64::MIN,
    });
    [Arg::Long(cursor.when_ms), Arg::Long(cursor.id)]
}

fn limit(limit: usize) -> Arg<'static> {
    Arg::Int(i32::try_from(limit).unwrap_or(i32::MAX))
}

/// A SIM as the glue takes it: `-1` (Android's invalid subscription id)
/// is the phone's default.
fn sim_arg(sim: Option<i32>) -> Arg<'static> {
    Arg::Int(sim.unwrap_or(-1))
}

// ---- events ----

/// `Events.open()`: start a watch (replacing any); returns its generation.
fn open_events() -> Result<i64, PlatformError> {
    jni::call("Events.open", |env, context| {
        jni::call_static(env, context, "Events", "open", "()J", &[])?.j()
    })
}

/// The next event's JSON for watch `generation`, blocking; `None` once it
/// is closed or replaced.
fn take(generation: i64) -> Result<Option<String>, PlatformError> {
    jni::call("Events.take", |env, context| {
        let json = jni::call_static(
            env,
            context,
            "Events",
            "take",
            "(J)Ljava/lang/String;",
            &[generation.into()],
        )?
        .l()?;
        if json.is_null() {
            return Ok(None);
        }
        Ok(Some(env.get_string(&JString::from(json))?.into()))
    })
}

/// Pull events into `on_event` on a thread of its own until the guard
/// drops. A new watch replaces the running one (its guard then stops
/// nothing). If `Events.take()` itself fails, the watch ends silently.
pub(super) fn watch(on_event: Box<dyn Fn(PhoneEvent) + Send>) -> Result<PhoneWatch, PlatformError> {
    let generation = open_events()?;
    // Without its permission a listener fails to arm (`rearm` once granted);
    // notes still flow.
    let _ = listen_calls(true, generation);
    let _ = listen_sms(true, generation);
    let pull = std::thread::Builder::new()
        .name("carrova-phone-events".into())
        .spawn(move || {
            // shortcut: a failing `take()` (JNI error) ends the watch with no
            // signal; report it to the owner once the trait has a way to.
            while let Ok(Some(json)) = take(generation) {
                // Malformed or unknown events are skipped: one bad note must
                // not end the watch.
                if let Ok(Some(event)) = events::parse(&json) {
                    on_event(event);
                }
            }
        });
    let pull = match pull {
        Ok(pull) => pull,
        Err(e) => {
            stop_events(generation);
            return Err(PlatformError::Android {
                call: "Events.open",
                cause: e.to_string(),
            });
        }
    };
    Ok(PhoneWatch::new(move || {
        // Closing (or a newer watch's open) unblocks `take`: the join is quick.
        stop_events(generation);
        // Dropped from inside the callback: the thread ends on its own.
        if pull.thread().id() != std::thread::current().id() {
            let _ = pull.join();
        }
    })
    .with_rearm(move |perm| match perm {
        PhonePerm::Calls => listen_calls(true, generation),
        PhonePerm::Sms => listen_sms(true, generation),
        _ => Ok(()),
    }))
}

fn listen_calls(on: bool, generation: i64) -> Result<(), PlatformError> {
    void(
        "Calls.listen",
        "Calls",
        "listen",
        &[Arg::Bool(on), Arg::Long(generation)],
    )
}

fn listen_sms(on: bool, generation: i64) -> Result<(), PlatformError> {
    void(
        "Sms.listen",
        "Sms",
        "listen",
        &[Arg::Bool(on), Arg::Long(generation)],
    )
}

/// Stop the call and SMS listeners and close the queue of watch `generation`; both
/// do nothing if a newer watch replaced it. Nothing to report to: the
/// watch is ending.
fn stop_events(generation: i64) {
    let _ = listen_calls(false, generation);
    let _ = listen_sms(false, generation);
    let _ = jni::call("Events.close", |env, context| {
        jni::call_static(
            env,
            context,
            "Events",
            "close",
            "(J)V",
            &[generation.into()],
        )?
        .v()
    });
}

// ---- notifications ----

/// The notifications shown now, newest first.
pub(super) fn active_notes() -> Result<Vec<Note>, PlatformError> {
    let json = text("Notify.active", "Notify", "active", &[])?;
    let mut notes: Vec<Note> = events::list("Notify.active", &json)?;
    notes.sort_by_key(|note| Reverse(note.when_ms));
    Ok(notes)
}

pub(super) fn note_reply(key: &str, reply: &str) -> Result<bool, PlatformError> {
    glue(
        "Notify.reply",
        "Notify",
        "reply",
        &[Arg::Str(key), Arg::Str(reply)],
        "Z",
        |_, v| v.z(),
    )
}

pub(super) fn note_dismiss(key: &str) -> Result<(), PlatformError> {
    void("Notify.dismiss", "Notify", "dismiss", &[Arg::Str(key)])
}

pub(super) fn note_open(key: &str) -> Result<(), PlatformError> {
    void("Notify.open", "Notify", "open", &[Arg::Str(key)])
}

// ---- SMS ----

pub(super) fn sms_threads(
    before: Option<Cursor>,
    max: usize,
    since_ms: Option<i64>,
) -> Result<Vec<SmsThread>, PlatformError> {
    // `Sms.threads` reads its last match once `limit` are found: none at 0.
    if max == 0 {
        return Ok(Vec::new());
    }
    let [ms, id] = page_bound(before);
    let args = [ms, id, limit(max), Arg::Long(since_ms.unwrap_or(i64::MIN))];
    let json = text("Sms.threads", "Sms", "threads", &args)?;
    events::list("Sms.threads", &json)
}

pub(super) fn sms_thread(
    id: i64,
    before: Option<Cursor>,
    max: usize,
) -> Result<Vec<SmsMessage>, PlatformError> {
    let [ms, from] = page_bound(before);
    let args = [Arg::Long(id), ms, from, limit(max)];
    let json = text("Sms.thread", "Sms", "thread", &args)?;
    events::list("Sms.thread", &json)
}

/// Contact numbers matching `query` (`Calls.contacts`).
pub(super) fn phone_contacts(query: &str, max: usize) -> Result<Vec<Contact>, PlatformError> {
    let json = text(
        "Calls.contacts",
        "Calls",
        "contacts",
        &[Arg::Str(query), limit(max)],
    )?;
    events::list("Calls.contacts", &json)
}

/// The active SIMs (`Sims.list`: empty without `READ_PHONE_STATE`).
pub(super) fn phone_sims() -> Result<Vec<Sim>, PlatformError> {
    events::list("Sims.list", &text("Sims.list", "Sims", "list", &[])?)
}

pub(super) fn sms_send(
    to: &str,
    body: &str,
    sim: Option<i32>,
) -> Result<SendOutcome, PlatformError> {
    let args = [Arg::Str(to), Arg::Str(body), sim_arg(sim)];
    match text("Sms.send", "Sms", "send", &args)?.as_str() {
        "sent" => Ok(SendOutcome::Sent),
        "no_signal" => Ok(SendOutcome::NoSignal),
        "failed" => Ok(SendOutcome::Failed),
        "unknown" => Ok(SendOutcome::Unknown),
        _ => Err(PlatformError::Android {
            call: "Sms.send",
            cause: "unknown outcome".into(),
        }),
    }
}

// ---- calls ----

pub(super) fn call_answer() -> Result<(), PlatformError> {
    void("Calls.answer", "Calls", "answer", &[])
}

pub(super) fn call_decline() -> Result<(), PlatformError> {
    void("Calls.decline", "Calls", "decline", &[])
}

pub(super) fn call_end() -> Result<(), PlatformError> {
    void("Calls.end", "Calls", "end", &[])
}

pub(super) fn call_mute(on: bool) -> Result<(), PlatformError> {
    void("Calls.mute", "Calls", "mute", &[Arg::Bool(on)])
}

pub(super) fn call_dial(number: &str, sim: Option<i32>) -> Result<(), PlatformError> {
    void(
        "Calls.dial",
        "Calls",
        "dial",
        &[Arg::Str(number), sim_arg(sim)],
    )
}

pub(super) fn call_speaker(on: bool) -> Result<(), PlatformError> {
    void("Calls.speaker", "Calls", "speaker", &[Arg::Bool(on)])
}

pub(super) fn call_history(
    before: Option<Cursor>,
    max: usize,
) -> Result<Vec<CallRecord>, PlatformError> {
    let [ms, id] = page_bound(before);
    let json = text("Calls.history", "Calls", "history", &[ms, id, limit(max)])?;
    events::list("Calls.history", &json)
}

/// The call now: ringing, active or idle.
pub(super) fn call_state() -> Result<CallState, PlatformError> {
    events::call_state(&text("Calls.state", "Calls", "state", &[])?)
}

// ---- permissions ----

/// Run `f` only if `perm` is granted, else [`PlatformError::PermissionDenied`];
/// a `SecurityException` from `f` (revoked meanwhile) maps to the same.
pub(super) fn guarded<T>(
    perm: PhonePerm,
    f: impl FnOnce() -> Result<T, PlatformError>,
) -> Result<T, PlatformError> {
    if phone_perm(perm)? != PermState::Granted {
        return Err(PlatformError::PermissionDenied(perm));
    }
    f().map_err(|e| events::denied_as(perm, e))
}

/// The name `Perms.kt` knows `p` by.
fn perm_name(p: PhonePerm) -> &'static str {
    match p {
        PhonePerm::NotificationAccess => "notification_access",
        PhonePerm::Sms => "sms",
        PhonePerm::Calls => "calls",
        PhonePerm::Contacts => "contacts",
        PhonePerm::BluetoothConnect => "bluetooth_connect",
        PhonePerm::Battery => "battery",
    }
}

pub(super) fn phone_perm(p: PhonePerm) -> Result<PermState, PlatformError> {
    perm_state("Perms.state", "state", p)
}

/// Shows Android's runtime-permission dialog; blocks until it is answered.
pub(super) fn request_phone_perm(p: PhonePerm) -> Result<PermState, PlatformError> {
    perm_state("Perms.request", "request", p)
}

/// Call `Perms.<method>(name)` and map the state string it returns.
fn perm_state(call: &'static str, method: &str, p: PhonePerm) -> Result<PermState, PlatformError> {
    match text(call, "Perms", method, &[Arg::Str(perm_name(p))])?.as_str() {
        "granted" => Ok(PermState::Granted),
        "denied" => Ok(PermState::Denied),
        "restricted" => Ok(PermState::Restricted),
        _ => Err(PlatformError::Android {
            call,
            cause: "unknown state".into(),
        }),
    }
}

pub(super) fn open_perm_settings(p: PhonePerm) -> Result<(), PlatformError> {
    void(
        "Perms.openSettings",
        "Perms",
        "openSettings",
        &[Arg::Str(perm_name(p))],
    )
}

pub(super) fn remind_access(p: PhonePerm) -> Result<(), PlatformError> {
    void("Perms.remind", "Perms", "remind", &[Arg::Str(perm_name(p))])
}
