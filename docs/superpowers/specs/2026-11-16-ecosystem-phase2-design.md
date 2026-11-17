# Ecosystem Phase 2: the phone on the laptop — notifications, messages, calls

Date: 2026-11-16. Status: implemented; exit gate pending. Parent design
(decisions, approaches, roadmap): `docs/superpowers/specs/2026-11-15-ecosystem-design.md`.
Phase 1 (what this builds on): `2026-11-15-ecosystem-phase1-design.md`.
Screens: `docs/mockups/gui-ecosystem-desktop.html`, `docs/mockups/gui-ecosystem-android.html`.
Plan: `docs/superpowers/plans/2026-11-16-ecosystem-phase2.md`.

This document narrows the parent design to Phase 2 and pins what the parent
left open for it. Where they differ, this one wins for Phase 2. Phase 1's
pinned values (link, lanes, policy file, asks, Android shell, bridge) hold
unless changed here.

## What Phase 2 delivers

A Linux laptop linked to an Android phone (Phase 1 link) on which the user can:

1. see the phone's notifications and reply inline where the app allows it,
   with a per-app list on the phone;
2. read SMS conversations and send texts;
3. see a call ring, answer, decline, end, mute and call back, and browse
   call history, all over the link with no Bluetooth (you talk on the
   phone, speaker turned on at answer);
4. optionally, after a one-time wizard, talk through the laptop's mic and
   speakers (Bluetooth Hands-Free), with keypad and "move to phone".

## Gate 0 (before any Phase 2 code)

1. **Phase 1 exit gate passed.** All real-hardware checks in
   `tasks/todo.md` "Ecosystem Phase 1 — exit gate" ticked by the user.
   CLAUDE.md "stay in the current phase" holds.
2. **Phase 1 open question settled:** `carrova link serve` announcing
   presence (`tasks/todo.md` note). The user decides; the decision is
   recorded in `docs/PLAN.md`.
3. **Spikes on real hardware** (results written into this file under
   "Spike results" before Part B starts). Each fixes a pinned value below or
   moves a feature out:
   - S1 Kotlin → Rust events by blocking pull (see "Android bridge").
   - S2 From the `LinkService` foreground service, with the app not in
     front: `TelecomManager.acceptRingingCall`, `endCall`, `placeCall`,
     `AudioManager` speaker on and mic mute, on Android 13, 14 and 15.
   - S3 Sideloaded APK on Android 13+: which of notification access,
     `READ_SMS`/`SEND_SMS`/`RECEIVE_SMS`, `READ_CALL_LOG`,
     `ANSWER_PHONE_CALLS` are blocked by "restricted settings" until the
     user allows them in App info; Play Protect warnings on install.
   - S4 Ubuntu 24.04 laptop + Android phone: oFono + PipeWire as
     Hands-Free (phone = audio gateway), 5-minute call, both directions
     clear; PipeWire's Hands-Free node for the phone appears when SCO
     connects and disappears when the phone takes the audio back. If S4 fails, "Calls, optional audio" moves out of Phase 2 into
     its own plan and the exit gate's audio check is dropped; the base
     calls ship regardless.

No ADR is needed: nothing here touches `carrova-crypto` or the transport
handshake. Bluetooth pairing is the OS's own. If a spike forces a change to
either, stop and write ADR 0004 for sign-off first.

## Out of Phase 2

Screens, keyboard/mouse, touchpad (Phase 3); Windows/macOS link adapters
beyond compiling (new platform methods return `Unsupported`; the calls
wizard shows "This computer can't take calls"); MMS and group MMS (text
SMS only); RCS and chat-app histories (chat apps reply through their
notification only); dual-SIM choice (the phone's default SMS/call SIM is
used); Carrova as default dialer or default SMS app (never); storing
messages, notifications or call history on the laptop's disk; CLI commands
for reading or sending phone data (the CLI only switches the features, see
"CLI").

## Link additions (pinned)

| Item | Value |
|---|---|
| New lane kinds | `Notify`, `Messages`, `Calls` (all `version: 1`) |
| Lane shape | one short-lived lane per message, as Phase 1 (`ring.rs` pattern: send, ack, finish) |
| Snapshot on link open | phone sends active notifications (≤ 50, newest first) and the current call state; threads are pulled by the laptop on demand. Size: icons are CBOR byte strings and share a 512 KiB budget per snapshot (icons of the oldest notes dropped first), so a maximal snapshot (50 notes at full text, title and ids) encodes under the 1 MiB `MAX_FRAME_LEN`; a test encodes that worst case |
| Paging cursor | `Cursor { when_ms, id }`; pages are newest first by (`when_ms` DESC, `id` DESC) and `before` is exclusive on the pair, so rows sharing a timestamp are neither skipped nor repeated. Used by `Threads`, `Thread` and `History` |
| Feature groups | `notifications`, `messages`, `calls` are **quiet** (On/Off) |
| Giving side | the phone: its switch decides what leaves it |
| Receiving side | the laptop: its switch decides what it accepts and shows |
| Actions back (reply, send SMS, answer, dial) | allowed only while both sides have the feature on; the phone checks its own policy before acting |
| Defaults for a new phone link | notifications on (pre-selected), messages off, calls off (parent design) |
| Defaults for an existing known computer | all three off; computers never give these features |
| Laptop storage | memory only, dropped on link close and forget; never written to disk |
| Frame cap | existing `MAX_FRAME_LEN`; app icons PNG ≤ 64 KiB, else dropped |

### Notifications

- `NotifyMsg::Posted(Note)`, `Removed { key }`, `Snapshot(Vec<Note>)` phone →
  laptop; `NotifyAction::Reply { key, text }`, `Dismiss { key }`,
  `Open { key }` (opens on the phone) laptop → phone.
- `Note { key, app_id, app_name, title, text, when, can_reply, icon:
  Option<Vec<u8>> }`. `key` is the Android notification key. Text ≤ 4 KiB,
  title ≤ 256 B, reply ≤ 4 KiB (longer is cut on the phone, refused on
  receipt).
- Not mirrored: ongoing notifications (music, navigation), Carrova's own,
  group summaries, apps switched off in the per-app list.
- **Per-app list** lives on the phone (giving side), one list for all linked
  laptops: `<state>/phone/notify-apps.json`, `0600`, `{ "version": 1,
  "apps": { "<package>": "on"|"off" }, "hide_codes": bool, "show_text":
  bool }`. New apps default on, except a bundled data table of
  banking/authenticator package ids (my call, kept in
  `crates/carrova-core/data/private-apps.txt`, reviewed like recipes) that
  default off.
- "Hide one-time codes" (default on): a note whose text matches the OTP
  rule (4–8 digits next to "code", "OTP", "verification", or a localized
  equivalent in the table) is sent as "<App>: one-time code" with no text.
- "Show message text" off: only app name and title go.
- Laptop side: "Only while I'm using this computer" (laptop setting) holds
  pop-ups while idle/locked. Locked laptop (`Platform::screen_lock`):
  only "<Phone>: N new notifications"; `Unknown` fails closed.
- Desktop pop-up via `tauri-plugin-notification` is **not** used for content
  (it lands in the OS notification history); Carrova shows its own pop-up
  window per mockup (`Phone notifications`: popup/reply), and the OS
  notification only says "<Phone>: new notification".

### Messages (SMS)

- Laptop → phone: `Threads { before: Option<Cursor> }` (pages of 30),
  `Thread { id, before: Option<Cursor> }` (pages of 50 messages), `Send { to: String
  (one number; group SMS is out of Phase 2), text, client_id }`.
- Phone → laptop: `ThreadsPage`, `ThreadPage`, `Incoming(Message)`,
  `Resync` (refetch threads and any open thread), `SendResult { client_id,
  Sent | NoSignal | Failed | Unknown }`. `Unknown` (send timed out after
  60 s, or only some parts of a long text went out) means "may have been
  sent": the laptop says "Check your phone before resending" and offers
  no one-click retry, so a person never gets a text twice.
- Incoming texts (`PhoneEvent::SmsReceived`) carry `id: 0` until the
  default SMS app stores them; such a message is provisional, never a page
  cursor or dedupe key, and the laptop replaces it by refetching the
  newest page of that thread. Its `when_ms` is the phone's receive time.
  The refetch runs after the message is stored, not on the broadcast:
  the phone watches the SMS provider (`ContentObserver` on
  `content://sms`; the callback URI may be null, so it is not used to
  find the thread). The phone keeps the set of pending threads: each
  `SMS_RECEIVED` adds its `thread_id` (from `getOrCreateThreadId`) with
  the receive time, and schedules the same check, because the default
  SMS app gets `SMS_DELIVER` first and may already have stored the row
  (and fired its provider change) before Carrova's `SMS_RECEIVED`. 500 ms
  after the last provider change or new pending entry it queries, for
  each pending thread, whether an inbox row dated at or after the receive
  time minus 5 s (slack for rows stored before the broadcast) exists, and emits one `PhoneEvent::SmsStored
  { thread_id }` per thread that has one, removing it from the set
  (entries older than 10 s are dropped). The phone forwards each as
  `SmsPush::Stored { thread_id }`; the laptop then refetches that
  thread's newest page and drops its provisional rows. A `Stored` for a
  thread with no provisional row refetches that thread only if the
  laptop has it loaded, and is otherwise ignored. If no `Stored` arrives within 10 s of
  the provisional message, the laptop refetches once anyway. Until then
  the provisional message shows normally in the thread and the
  conversation list.
- "Keep in sync" (phone, mockup): 30 days default / 1 year / All decides how
  far back pages go. The laptop asks for pages; nothing is pushed in bulk.
- Contacts names resolved on the phone (`READ_CONTACTS`) and sent with the
  thread; the laptop never gets the address book. Compose suggestions come
  from recent thread names only (my call: no contacts sync).
- RCS note on the phone's Messages screen (state `rcs`), and the desktop
  Messages empty/thread screen says chat apps reply from notifications.

### Calls, base (no Bluetooth)

- Phone → laptop: `CallState::{ Idle, Ringing { number, name }, Active {
  since, muted, on_laptop_audio }, Ended { missed } }`, `HistoryPage`.
- Laptop → phone: `Answer`, `Decline`, `End`, `Mute(bool)`, `Dial {
  number }`, `History { before: Option<Cursor> }` (pages of 50, `READ_CALL_LOG`;
  each `CallRecord` carries its call-log `id`).
- Answer from the laptop turns the phone's speaker on (unless optional
  audio is set up and connected: then audio goes to the laptop).
  `on_laptop_audio` means a Bluetooth Hands-Free device can carry the call
  (Android can't tell the laptop's link from a headset).
- The laptop's audio side (`AudioSide`, shown in the call window) is the
  active route, not setup readiness or oFono's card (a card is only an
  attached device; the SCO audio link is separate). `Laptop` while an SCO
  link for the linked phone is observed; `Phone` otherwise. PipeWire is
  oFono's audio agent and owns the SCO socket, so Carrova can't take
  `NewConnection` itself; it watches PipeWire instead: a Hands-Free node
  for the phone's Bluetooth address appears when SCO connects and goes
  away when SCO drops (the phone takes the audio back, or goes out of
  range). When PipeWire can't be read, the window shows `Phone` with "Use
  this computer's mic and speakers". To be verified in spike S4.
- Ringing on the laptop follows the laptop setting "Ring on this
  computer": Always / While I'm using it (default) / Never (mockup
  Settings › Calls and Bluetooth). "Also ring this phone" is the phone's own
  ringer, untouched.
- The laptop's ringing window is a separate always-on-top Tauri window
  (mockup Incoming call); missed calls become a note with "Message" and
  "Call back".
- Numbers are never logged; error reports redact digit runs ≥ 5 (added to
  the `carrova-report` redaction table).

### Calls, optional audio (Bluetooth Hands-Free, Linux)

- The laptop acts as a Hands-Free unit; the phone stays the audio gateway.
  Linux path: BlueZ + oFono + PipeWire (WirePlumber `bluez5.hfphsp-backend
  = "ofono"`, role `hfp_hf`).
- Wizard (mockup Calls setup: intro → pair → test → done; off; nobt) plus
  two new states the plan adds to the desktop mockup:
  - **`install` "Install one package"**: shown when oFono is missing or
    Carrova's WirePlumber drop-in is not in place;
    one button installs `ofono` through PackageKit (`org.freedesktop.
    PackageKit`, the OS asks for the password); Ubuntu 24.04 LTS and newer.
    Then Carrova writes one WirePlumber drop-in (0.4: Lua file under
    `~/.config/wireplumber/bluetooth.lua.d/`; 0.5+: `~/.config/
    wireplumber/wireplumber.conf.d/51-carrova-hfp.conf`) and restarts the
    user's WirePlumber service, after the user agrees on that screen.
    "Set up again" / turning calls off removes the drop-in (reversible).
    The install gives up after 10 min ("Installing didn't finish. Try
    again") and a dismissed password prompt reads "Install cancelled".
  - **`phone` "Talk on the phone"**: shown at the end of the wizard's intro
    as the always-available fallback ("You can answer here and talk on
    the phone, no setup needed"), and as the state of the device row when
    audio is not set up.
- Bluetooth pairing through BlueZ D-Bus (`Adapter1.StartDiscovery`,
  `Device1.Pair`, an `Agent1` showing the passkey per mockup `pair`).
- Keypad: oFono `VoiceCallManager.SendTones`. Move to phone: BlueZ
  `Device1.DisconnectProfile(HFP_AG)` on the phone (Android routes the
  call back to its earpiece when the hands-free link drops; oFono has no
  audio disconnect). Move to laptop: nothing if PipeWire already has the
  phone's hands-free node; else `DisconnectProfile(HFP_AG)` if the link is
  up without audio, then `ConnectProfile(HFP_AG)`, and the phone brings the
  call's audio over on reconnect. oFono accepts
  `HandsfreeAudioCard.Connect`/`Acquire` only from its audio agent
  (PipeWire), which acquires the link only for a node it already has.
  Both to be verified in spike S4.
- Pairing with several discoverable phones in range reports "several
  phones visible" (the wizard asks to make only your phone visible);
  targeting the linked phone directly is a later change.
- States from `Platform::call_audio_status`: `NoAdapter`, `BluetoothOff`,
  `NoHandsFree` (= mockup `nobt`; only when oFono runs and is configured
  and the adapter still offers no Hands-Free role, never for a software
  state), `NeedsPackage` (= `install`: oFono missing or drop-in missing),
  `NeedsPairing`, `Ready`, `Unsupported` (non-Linux).
- D-Bus through `zbus` (already a `carrova-platform` dependency), hand-written
  proxies; no new crate.

## Android bridge (pinned)

- Phase 1 has only Rust → Kotlin calls. Phase 2 needs Kotlin → Rust events
  (notification posted, SMS received, call state). **My call: pull, not
  push.** Kotlin keeps one bounded `LinkedBlockingQueue<String>` (cap 256,
  oldest dropped) of JSON event strings in `platform/Events.kt`; Rust
  calls `Events.take()` from one thread owned by
  `Platform::watch_phone_events`, blocking with no timeout; the watch
  guard's drop calls `Events.close()`, which puts a sentinel. No new JNI
  exports, no new `unsafe`, ADR 0003 unchanged.
- New Kotlin glue (no decisions in it): `NotifyListener.kt`
  (`NotificationListenerService`), `Notify.kt` (reply via `RemoteInput`,
  dismiss, open), `Sms.kt` (content provider reads, `SmsManager` send,
  `SMS_RECEIVED` receiver), `Calls.kt` (`PhoneStateListener`, which still carries the ringing
  number that `TelephonyCallback` withholds from third-party apps; `TelecomManager`,
  `AudioManager`, `CallLog`), `Perms.kt` (status of each permission, open the
  right settings page), `Events.kt`.
- Permissions are requested only when the matching feature is turned on:
  notifications → notification access (settings page); messages →
  `READ_SMS`, `SEND_SMS`, `RECEIVE_SMS`, `READ_CONTACTS`; calls →
  `READ_PHONE_STATE`, `READ_CALL_LOG`, `ANSWER_PHONE_CALLS`, `CALL_PHONE`,
  `READ_CONTACTS`; optional audio → `BLUETOOTH_CONNECT`.
- **Restricted settings step** (Android 13+, sideloaded): before a
  permission S3 shows to be restricted, the phone shows "Allow restricted
  settings" (App info → ⋮ → Allow restricted settings) with a button that
  opens App info. The plan adds this state to the Android mockup (the
  comment at L1080 asks for it) plus the notification-access and phone
  permission states the onboarding lacks.

## Policy and UI changes (pinned)

- `Feature::group` returns `Quiet` for `Notifications`, `Messages`, `Calls`;
  `Feature::PHASE2` = Phase 1 + those three; the UI shows their rows.
  `Screen` and `Input` stay reserved.
- Mockup copy fix: the desktop device-page "Calls" row says "Ring and
  answer here. Uses Bluetooth, set up once." That contradicts the user's
  two-step calls decision. New copy: "Ring and answer here" with a
  secondary "Use this computer's mic and speakers · Set up". Android Calls
  `setup` likewise leads with base calls, Bluetooth as the optional step.
- Desktop screens built: device page rows and `missing` state, Phone
  notifications (popup, reply, apps, locked, missing), Messages (thread,
  compose, failed, empty, permission, away), Incoming call (ringing,
  missed), In a call (active, muted, keypad, moved), Calls (log, empty,
  notset), Calls setup (intro, phone, install, pair, test, done, off, nobt),
  Home `call` bar, Tray `call`, Settings › Calls and Bluetooth.
- Android screens built: Choose what to share (Phase 2 rows, `sms`,
  `battery`, new `restricted`, `access`, `phoneperms`), device page rows,
  Notifications on the laptop (apps, noaccess), Messages (synced, first,
  noperm, rcs), Calls (setup, pairing, ready, active, btoff).

## CLI (pinned)

`carrova devices features <device> notifications=on|off messages=on|off
calls=on|off` works through the existing command; `carrova link status`
shows the new rows. Nothing else (my call: reading texts and notifications
is a UI job; skipped until someone asks).

## Privacy (pinned)

- Never logged: notification title/text, app names, message text, numbers,
  contact names, call state details. `tracing` fields carry counts and ids
  only.
- `carrova-report` redaction gains digit runs ≥ 5 and `+` numbers; a test
  feeds a sample of each phone-data type through a failing path and asserts
  the report holds none of it.
- Laptop keeps phone data in memory only; forget and link close clear it.

## Acceptance (Phase 2 exit gate)

Automated (`just ci` green, `just e2e --link` PASS, output shown):
- Lane wire round-trips and limits for `Notify`, `Messages`, `Calls`;
  unknown versions refused.
- Two supervisors in one process with a fake phone platform: notifications
  flow only when both sides allow and the app is on; reply reaches the fake;
  OTP hidden; locked laptop shows count only; SMS pages, send result, failed
  send; call ring → answer → end, decline, missed, dial; forget clears
  laptop memory.
- Linux call-audio status parsing from recorded D-Bus replies; WirePlumber
  drop-in write/remove for 0.4 and 0.5.
- Redaction test above.

Real hardware (user, Ubuntu 24.04 laptop + Android 13+ phone):
- Reply to a chat-app notification from the laptop.
- Receive an SMS on the laptop and reply; send a new SMS.
- With no Bluetooth: answer and end a call from the laptop (talk on phone,
  speaker on); decline; call back from history.
- With audio set up on Ubuntu 24.04: hold a 5-minute call on the laptop,
  clear both ways; keypad digits reach an IVR; move to phone and back.
- Idle phone battery over 8 h with notifications + messages + calls on:
  Carrova below 2 % of total.

## Spike results

- **S1 (2026-11-16, API 34 emulator, debug build): passed.** Blocking pull
  through the existing `jni.rs` helpers, no new export, no new `unsafe`,
  no `jni.rs` change. 1 000 events 1 ms apart: offer→receive p50 196 µs,
  p99 1.4 ms; `close()` unblocks the Rust thread in ~1 ms; blocked thread
  0 CPU over 55 s idle (`futex_wait`). Class loading from the attached
  thread works (context class loader). Two amendments, pinned:
  `Events.open()` clears the queue and the close sentinel when a watch
  starts (a second watch in one process would otherwise end at once); and
  on overflow Kotlin enqueues one `{"type":"dropped","count":n}` event,
  which Rust maps to `PhoneEvent::Dropped { count }`. Any event type can be
  lost, so the phone resyncs every feature to each linked laptop that
  allows it: the notification `Snapshot`, the current call state (read
  with `Platform::call_state`, so a lost `Ended` cannot leave a stale
  call on the laptop) and `SmsPush::Resync` (the laptop refetches its
  thread list and any open thread). Reference code: branch `spike/phase2` `a984c27`
  (never merged).
- S2–S4: pending (real hardware, user).

## Left to the builder

- CBOR field layout inside the pinned messages, exact page sizes within
  the caps above.
- OTP localization table contents (English + the languages the UI ships).
- Ringtone sound for the laptop ringing window (bundled, CC0).
- Order of permission prompts inside each mocked state.
