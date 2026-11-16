# Ecosystem Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A linked Android phone shows up on the Linux laptop: notifications with inline reply, SMS conversations and compose, calls rung, answered and ended over the link, and optionally call audio through the laptop over Bluetooth Hands-Free.

**Architecture:** Three new quiet features (`notifications`, `messages`, `calls`) and lanes (`Notify`, `Messages`, `Calls`) in `carrova-link`, using phone data types defined once in `carrova-platform::phone`. Android gains Kotlin glue for the listener, SMS and telephony, reached Rust → Kotlin as in ADR 0003, with Kotlin → Rust events pulled by Rust from a blocking queue (no new JNI export). `carrova-core::ecosystem` gains a phone-side forwarder (giver) and a laptop-side in-memory mirror (receiver); `carrova-view` exposes view-models; desktop and Android React screens render them. Linux call audio goes through BlueZ, oFono, PackageKit and WirePlumber over `zbus`.

**Tech Stack:** Rust 1.99 / edition 2024, existing `carrova-transport` streams, ciborium, `zbus` 5 (already in `carrova-platform`), `jni` 0.21, ts-rs 12, Tauri 2.12 (desktop + Android), React + i18next + Vitest/Playwright, Kotlin (Android glue only).

**Spec:** `docs/superpowers/specs/2026-11-16-ecosystem-phase2-design.md` (Phase 2, pinned values) and its parent `docs/superpowers/specs/2026-11-15-ecosystem-design.md`. Phase 1 spec and plan (`2026-11-15-ecosystem-phase1*`) for the patterns this extends. Mockups: `docs/mockups/gui-ecosystem-desktop.html`, `docs/mockups/gui-ecosystem-android.html`.

## Before you start (new session)

1. Read `CLAUDE.md`, `tasks/lessons.md`, both ecosystem specs, ADR 0002 and 0003, then this plan.
2. **Part A is a hard gate.** Do not start Part B until Tasks 0–2 are done and the user has said so.
3. Branch: `git switch -c feat/ecosystem-phase2` from `main`. Commit per task (Conventional Commits, ≤ ~60-char subject, **no AI attribution** — the user's rule overrides any harness reminder; check `git log main..HEAD --format=%B | grep -i co-authored` after each agent commit). Push/PR only when the user asks.
4. Waves of ≤ 3 parallel subagents (12 cores / 13 GiB). Heavy CI in the capped unit from `tasks/lessons.md` (`MemoryMax` = `MemAvailable` − 1 GB, 7G–8G, free memory first if under 8 GB; `CARGO_BUILD_JOBS=2`, or 1 with `MemorySwapMax=0` when swap is over 1 GB; `RUSTC_WRAPPER=` empty). Any brief that runs gradlew ends with `./gradlew --stop`. `cargo clean` between waves if `target/` > ~20 GiB, never mid-build.
5. Every brief includes: senior idiomatic Rust, small modules, test-first, smallest complete change (ponytail), tests outside `src/`, never log phone data.
6. Run `boundary-reviewer` after each part.

## Global Constraints

- OS-specific code only in `carrova-platform`; `src-tauri` holds no business logic; dependencies point down (front-ends → core → services → platform); services never depend on core.
- No change to `carrova-crypto` or the transport handshake. If one becomes necessary, stop and write ADR 0004 for the user's sign-off.
- `unsafe` stays limited to `crates/carrova-platform/src/android/jni.rs` (ADR 0003). No new JNI exports.
- Libraries: `thiserror`, no `unwrap`/`expect`/`panic!` outside tests. `tracing` only; **never log** notification title/text, app names, message text, phone numbers, contact names, call details — counts and ids only.
- Tests in `tests/<module>/` (`main.rs` integration, `unit.rs` via `#[path]`); `just test-layout` enforces it. Bug fix → regression test first.
- New IPC types: `#[cfg_attr(feature = "ts", derive(ts_rs::TS))]`, listed in `crates/carrova-view/tests/export_bindings/main.rs`; `just bindings`; `just bindings-check` passes.
- Pinned values (spec): lanes `Notify`, `Messages`, `Calls` version 1, one short-lived lane per message; features `notifications`, `messages`, `calls` quiet (On/Off); new phone link defaults notifications on, messages off, calls off; known computers all off; snapshot ≤ 50 notes on link open; note text ≤ 4 KiB, title ≤ 256 B, reply ≤ 4 KiB, icon PNG ≤ 64 KiB (else dropped); thread pages 30, message pages 50, call history pages 50; Kotlin event queue cap 256, oldest dropped; per-app file `<state>/phone/notify-apps.json` mode `0600` `{ "version": 1, "apps": {…}, "hide_codes": true, "show_text": true }`; private-app table `crates/carrova-core/data/private-apps.txt`; sync window 30 days default / 1 year / All; "Ring on this computer" Always / While I'm using it (default) / Never; WirePlumber drop-ins `~/.config/wireplumber/bluetooth.lua.d/51-carrova-hfp.lua` (0.4) and `~/.config/wireplumber/wireplumber.conf.d/51-carrova-hfp.conf` (0.5+); package `ofono` via PackageKit; laptop keeps phone data in memory only.
- Done = `just ci` green with output shown, plus `just e2e --link` PASS (no regression).

## Review Focus

1. **Link drops while a call rings or is active:** the laptop ringing window closes and the call bar shows "Pixel 8 left", never rings forever. → Task 13 `ringing_clears_when_link_drops`.
2. **Reply to a notification already removed on the phone:** the phone answers `Gone`; the laptop shows "Already dismissed on phone" and drops the note. → Task 4 `reply_result_roundtrip`, Task 11 `reply_to_removed_note_reports_gone`.
3. **SMS send retried or double-clicked, or sent while the phone is away:** one SMS per `client_id`; away shows "Sending works when Pixel 8 is back" and nothing is queued silently. → Task 12 `send_twice_same_client_id_sends_once`, `send_while_away_refused`.
4. **Notification flood** (busy group chat, 100+/s): the Kotlin queue drops oldest past 256; the laptop coalesces pop-ups per app to one per 2 s, the list stays complete. → Task 8 `queue_drops_oldest_past_cap` (Kotlin unit), Task 12 `burst_coalesces_per_app`.
5. **A feature switched off (either side) or the device forgotten while the laptop holds phone data:** that feature's mirror is cleared at once and its windows close. → Task 12 `feature_off_clears_mirror`, `forget_clears_all_phone_data`.

---

## Part A — Gate (no Phase 2 code)

### Task 0: Close Phase 1

- [ ] **Step 1:** Ask the user to run the "Ecosystem Phase 1 — exit gate" hardware checklist in `tasks/todo.md`; tick each item they report. Fix any failure on a `fix/` branch first (regression test first).
- [ ] **Step 2:** Ask the user to decide the open question "`carrova link serve` does not announce presence" (options: announce from `link serve`; or document CLI-to-CLI links as unsupported). Record the decision in `docs/PLAN.md` Ecosystem section; implement it if chosen (own small task, own commit).
- [ ] **Step 3:** Mark Phase 1 complete in `docs/PLAN.md` (table + status line) and `CLAUDE.md` Status. Commit `docs: close ecosystem phase 1`.

### Task 1: Hardware spikes S1–S4

Throwaway code on a `spike/phase2` branch, never merged. Results go into the spec's "Spike results" section.

- [ ] **S1 (emulator ok):** Kotlin `Events` queue + Rust thread calling `Events.take()` through `jni.rs` helpers; post 1 000 events from Kotlin; record latency and that `close()` unblocks the thread. Record idle CPU of the blocked thread (expect 0).
- [ ] **S2 (real phone, Android 13/14/15 where available):** from `LinkService` with the app swiped away: `TelecomManager.acceptRingingCall()`, `endCall()`, `placeCall()`, `AudioManager` speaker on (`setCommunicationDevice` on 31+) and `setMicrophoneMute(true)`. Record which work; any that fails becomes a "do on phone" button in the UI (spec amendment, user told).
- [ ] **S3 (real phone, sideloaded release APK):** for each permission in the spec's Android bridge list, does the request work directly or need "Allow restricted settings" first? Play Protect prompts on install?
- [ ] **S4 (Ubuntu 24.04 + phone):** install `ofono`, WirePlumber drop-in, pair the phone, place a 5-minute call, check audio both ways, `SendTones`, audio card disconnect/connect. Record WirePlumber version and the exact drop-in that worked.
- [ ] **Step 5:** Write results into the spec. If S4 failed: move "Calls, optional audio" to a later plan (strike Tasks 9 and 17 Part 2, the audio exit-gate line), tell the user. Commit `docs(spec): record phase 2 spike results`.

### Task 2: Mockup states and copy fix

**Files:** Modify `docs/mockups/gui-ecosystem-desktop.html`, `docs/mockups/gui-ecosystem-android.html`

- [ ] **Step 1:** Desktop: add Calls setup states `phone` ("Talk on the phone", no setup) and `install` ("Install one package", one button, password note, then "Carrova will change one audio setting" agree step); change the device-page Calls row copy per the spec ("Ring and answer here" + secondary "Use this computer's mic and speakers · Set up").
- [ ] **Step 2:** Android: add Choose-what-to-share states `restricted` (Allow restricted settings, App info button), `access` (notification access), `phoneperms` (calls + contacts); reorder Calls `setup` to lead with base calls, Bluetooth as optional. Remove the L1080 "Phase 2 note" comment.
- [ ] **Step 3:** Show the user the new states (open both files); get approval. Commit `docs(mockups): add phase 2 setup and permission states`.

---

## Part B — `carrova-link` and phone data types

### Task 3: Phone data types, features, lane kinds

**Files:**
- Create: `crates/carrova-platform/src/phone.rs` (re-exported from `lib.rs`)
- Modify: `crates/carrova-link/src/policy.rs`, `src/lane.rs`, `Cargo.toml` (depend on `carrova-platform` for the types; downward, allowed)
- Modify: `apps/desktop/src/screens/devices/model.ts`, `apps/desktop/src/mobile/model.ts`, `apps/desktop/src/mobile/pairing/ChooseShare.tsx`, `crates/carrova-core/src/ecosystem/peer.rs` (switch `PHASE1` → `PHASE2` where rows are listed)
- Test: `crates/carrova-platform/tests/phone/main.rs`, `crates/carrova-link/tests/policy/main.rs`, `crates/carrova-link/tests/lane/main.rs`, `apps/cli/tests/…` (existing `devices features` test file)

**Interfaces:**
- Produces (all `Serialize + Deserialize + Clone + Debug + PartialEq`, `ts` derive):
  - `pub struct Note { pub key: String, pub app_id: String, pub app_name: String, pub title: String, pub text: String, pub when_ms: i64, pub can_reply: bool, pub icon: Option<Vec<u8>>, pub hidden: Option<Hidden> }`, `pub enum Hidden { Code, TextOff, Locked }` (set by core filters; the UI picks the copy: "one-time code", title only, "N new notifications")
  - `pub struct SmsThread { pub id: i64, pub name: Option<String>, pub address: String, pub snippet: String, pub when_ms: i64, pub unread: u32 }`
  - `pub struct SmsMessage { pub id: i64, pub thread_id: i64, pub outgoing: bool, pub text: String, pub when_ms: i64 }`
  - `pub enum CallState { Idle, Ringing { number: String, name: Option<String> }, Active { since_ms: i64, muted: bool, on_laptop_audio: bool }, Ended { missed: bool } }`
  - `pub struct CallRecord { pub id: i64, pub number: String, pub name: Option<String>, pub when_ms: i64, pub duration_s: u32, pub kind: CallKind }`, `pub enum CallKind { Incoming, Outgoing, Missed, Declined }`
  - `pub struct Cursor { pub when_ms: i64, pub id: i64 }` (paging cursor; pages newest first by (when_ms, id) DESC, `before` exclusive on the pair)
  - `pub enum PhoneEvent { NotePosted(Note), NoteRemoved { key: String }, SmsReceived(SmsMessage), SmsStored { thread_id: i64 }, Call(CallState), Dropped { count: u32 } }` (`Dropped` = Kotlin queue overflowed; spec S1 result)
  - `pub enum SendOutcome { Sent, NoSignal, Failed }`, `pub enum AudioSide { Phone, Laptop }` (defined here once; `carrova-link` re-exports them)
  - `pub enum PhonePerm { NotificationAccess, Sms, Calls, Contacts, BluetoothConnect }`, `pub enum PermState { Granted, Denied, Restricted }`
  - `Feature::group` → `Quiet` for `Notifications | Messages | Calls`; `pub const PHASE2: [Feature; 7]`; `PolicyDoc::phone_defaults()` adds notifications On, messages Off, calls Off; `computer_defaults()` unchanged (all Off).
  - `LaneKind::{Notify, Messages, Calls}`.
- [ ] **Step 1: Failing tests:** `phase2_features_are_quiet`, `phone_defaults_match_spec` (JSON equals `{"version":1,"features":{"clipboard":"on","files":"ask","handoff":"ask","ring":"on","notifications":"on","messages":"off","calls":"off"}}`), `screen_and_input_stay_reserved`, `new_lane_kinds_roundtrip`, `phone_types_cbor_roundtrip`, CLI `devices_features_sets_notifications`.
- [ ] **Step 2–4:** FAIL → implement → PASS (`cargo test -p carrova-platform -p carrova-link -p carrova-cli`).
- [ ] **Step 5: Commit** `feat(link): activate notifications, messages and calls`.

### Task 4: Notify lane

**Files:** Create `crates/carrova-link/src/notify.rs`; Test `crates/carrova-link/tests/notify/main.rs`

**Interfaces:**
- Produces: `pub enum NotifyMsg { Posted(Note), Removed { key: String }, Snapshot(Vec<Note>) }`; `pub enum NotifyAction { Reply { key: String, text: String }, Dismiss { key: String }, Open { key: String } }`; `pub enum ActionResult { Done, Gone, Failed }`; consts `MAX_NOTE_TEXT = 4096`, `MAX_NOTE_TITLE = 256`, `MAX_REPLY = 4096`, `MAX_ICON = 65536`, `MAX_SNAPSHOT = 50`; `Note::fit(self) -> Note` (cuts text/title on char boundary, drops oversize icon); `send_notify(conn, &NotifyMsg) -> Result<(), LinkError>`, `recv_notify(stream) -> Result<NotifyMsg, LinkError>` (refuses over-limit with `TooBig`); `send_action(conn, &NotifyAction) -> Result<ActionResult, LinkError>`, `recv_action(stream) -> Result<(NotifyAction, Responder), LinkError>` with `Responder::reply(self, ActionResult)`.
- Wire: first frame after `LaneOpen` is an enum `NotifyFrame { Msg(NotifyMsg), Action(NotifyAction) }` so one lane kind carries both directions.
- [ ] **Step 1: Failing tests:** `posted_roundtrip`, `snapshot_capped_at_50`, `oversize_text_refused_on_receive`, `fit_cuts_on_char_boundary` (multibyte at the 4096 edge), `oversize_icon_dropped`, `reply_result_roundtrip` (`Gone` reaches the sender).
- [ ] **Step 2–5:** FAIL → implement → PASS → commit `feat(link): add notification lane`.

### Task 5: Messages lane

**Files:** Create `crates/carrova-link/src/messages.rs`; Test `crates/carrova-link/tests/messages/main.rs`

**Interfaces:**
- Produces: `pub enum SmsRequest { Threads { before: Option<Cursor> }, Thread { id: i64, before: Option<Cursor> }, Send { to: String, text: String, client_id: u64 } }`; `pub enum SmsReply { ThreadsPage(Vec<SmsThread>), ThreadPage(Vec<SmsMessage>), SendResult { client_id: u64, outcome: SendOutcome } }`; `SendOutcome` from Task 3; `pub enum SmsPush { Incoming(SmsMessage), Stored { thread_id: i64 }, Resync }`; consts `THREAD_PAGE = 30`, `MESSAGE_PAGE = 50`, `MAX_SMS_TEXT = 4096`; `request_sms(conn, &SmsRequest) -> Result<SmsReply, LinkError>`, `serve_sms(stream) -> Result<(SmsRequest, Responder), LinkError>`, `push_sms(conn, &SmsPush)`, `recv_sms_push(stream)`.
- [ ] **Step 1: Failing tests:** `threads_page_roundtrip`, `page_over_cap_refused`, `send_result_roundtrip`, `send_text_over_cap_refused`, `incoming_push_roundtrip`.
- [ ] **Step 2–5:** FAIL → implement → PASS → commit `feat(link): add SMS lane`.

### Task 6: Calls lane

**Files:** Create `crates/carrova-link/src/calls.rs`; Test `crates/carrova-link/tests/calls/main.rs`

**Interfaces:**
- Produces: `pub enum CallCommand { Answer, Decline, End, Mute(bool), Dial { number: String }, History { before: Option<Cursor> }, Tones(String), AudioTo(AudioSide) }` (`AudioSide` from Task 3); `pub enum CallReply { Done, NotRinging, NoCall, Failed, HistoryPage(Vec<CallRecord>) }`; `send_call_state(conn, &CallState)`, `recv_call_state(stream)`, `call_command(conn, &CallCommand) -> Result<CallReply, LinkError>`, `serve_call_command(stream) -> Result<(CallCommand, Responder), LinkError>`; `HISTORY_PAGE = 50`; `Dial` accepts only `[0-9+*#]`, length ≤ 32 (`LinkError::Protocol("number")` else); `Tones` same alphabet, ≤ 32.
- `Tones` and `AudioTo` are answered by the laptop's own platform (Task 9), not sent to the phone; they live here so the call controller has one command type.
- [ ] **Step 1: Failing tests:** `state_roundtrip`, `dial_rejects_letters_and_long`, `history_page_roundtrip`, `answer_when_idle_returns_not_ringing` (serve side test double).
- [ ] **Step 2–5:** FAIL → implement → PASS → commit `feat(link): add calls lane`.

---

## Part C — Platform

### Task 7: Phone platform methods and fakes

**Files:**
- Modify: `crates/carrova-platform/src/platform.rs` (new section "Ecosystem Phase 2", default bodies `Unsupported`)
- Modify: `crates/carrova-core/tests/ecosystem/supervisor/recording.rs` (fake phone: scripted events, recorded actions)
- Test: `crates/carrova-platform/tests/platform/…` (defaults return `Unsupported`)

**Interfaces:**
- Produces on `trait Platform`:
  - `fn watch_phone_events(&self, on_event: Box<dyn Fn(PhoneEvent) + Send>) -> Result<PhoneWatch, PlatformError>` (guard; drop stops)
  - `fn active_notes(&self) -> Result<Vec<Note>, PlatformError>`
  - `fn note_reply(&self, key: &str, text: &str) -> Result<bool, PlatformError>` (`false` = gone); `fn note_dismiss(&self, key: &str)`; `fn note_open(&self, key: &str)`
  - `fn sms_threads(&self, before: Option<Cursor>, limit: usize, since_ms: Option<i64>) -> Result<Vec<SmsThread>, PlatformError>`; `fn sms_thread(&self, id: i64, before: Option<Cursor>, limit: usize) -> Result<Vec<SmsMessage>, PlatformError>`; `fn sms_send(&self, to: &str, text: &str) -> Result<SendOutcome, PlatformError>`
  - `fn call_answer(&self)`, `fn call_decline(&self)`, `fn call_end(&self)`, `fn call_mute(&self, on: bool)`, `fn call_dial(&self, number: &str)`, `fn call_speaker(&self, on: bool)`, all `-> Result<(), PlatformError>`; `fn call_history(&self, before: Option<Cursor>, limit: usize) -> Result<Vec<CallRecord>, PlatformError>`
  - `fn call_state(&self) -> Result<CallState, PlatformError>` (current call; `Idle` when none; used to resync after `Dropped`)
  - `fn phone_perm(&self, p: PhonePerm) -> Result<PermState, PlatformError>`; `fn open_perm_settings(&self, p: PhonePerm) -> Result<(), PlatformError>`
  - Laptop audio: `fn call_audio_status(&self) -> Result<CallAudioStatus, PlatformError>` with `pub enum CallAudioStatus { NoAdapter, BluetoothOff, NoHandsFree, NeedsPackage, NeedsPairing, Ready, Unsupported }`; `fn install_call_audio(&self) -> Result<(), PlatformError>`; `fn set_call_audio_config(&self, on: bool) -> Result<(), PlatformError>`; `fn bt_pair(&self, on_passkey: Box<dyn Fn(u32) + Send>) -> Result<(), PlatformError>`; `fn call_tones(&self, digits: &str)`; `fn call_audio_to(&self, side: AudioSide)`
- [ ] **Step 1: Failing test** `phase2_defaults_unsupported` (each method on a bare adapter → `Unsupported("<name>")`).
- [ ] **Step 2–4:** FAIL → implement → PASS; extend `Recording` with `script_event(PhoneEvent)` and `actions() -> Vec<String>` (method names + ids only).
- [ ] **Step 5: Commit** `feat(platform): add phone and call audio methods`.

### Task 8: Android adapter and Kotlin glue

**Files:**
- Create: `crates/carrova-platform/src/android/{phone.rs,events.rs}`; Kotlin `apps/desktop/src-tauri/gen/android/app/src/main/java/org/byteveda/carrova/platform/{Events.kt,NotifyListener.kt,Notify.kt,Sms.kt,Calls.kt,Perms.kt}`
- Modify: `crates/carrova-platform/src/android/platform.rs` (wire methods), `AndroidManifest.xml` (permissions from the spec's list, `NotifyListener` service with `BIND_NOTIFICATION_LISTENER_SERVICE`, `SMS_RECEIVED` receiver)
- Test: `crates/carrova-platform/tests/android/events/unit.rs` (JSON event parsing, host-run), Kotlin unit test `app/src/test/…/EventsTest.kt`

**Interfaces:**
- Consumes: Task 7 trait, `jni.rs` call helpers (Phase 1).
- Produces: `events::parse(json: &str) -> Result<PhoneEvent, PlatformError>` (pure, host-testable; unknown `type` → skipped with a counter, not an error). Kotlin `Events.open()` (clears queue and close sentinel; called when a watch starts), `Events.offer(json)`, `Events.take(): String?` (`null` after `close()`), `Events.close()`; queue cap 256, oldest dropped, and on overflow one `{"type":"dropped","count":n}` event is enqueued (coalesced while it is still queued). Reference implementation: branch `spike/phase2` `a984c27` (spike; rewrite, don't cherry-pick).
- Kotlin rules: no decisions — filtering (per-app, OTP, ongoing) is Rust's job (Task 11) except dropping `FLAG_ONGOING_EVENT`, group summaries and Carrova's own notes, which Kotlin cannot defer cheaply. Speaker on at answer is Rust's call (`call_speaker`), not Kotlin's.
- [ ] **Step 1: Failing tests:** Rust `parses_each_event_type`, `unknown_type_skipped`, `bad_json_is_error`; Kotlin `queue_drops_oldest_past_cap`, `overflow_enqueues_one_dropped_event`, `close_unblocks_take`, `open_after_close_resets`.
- [ ] **Step 2–4:** FAIL → implement → PASS (`cargo test -p carrova-platform`; `./gradlew :app:testDebugUnitTest --max-workers=2 -Dorg.gradle.jvmargs=-Xmx1536m`; then `./gradlew --stop`). `just android-check` (or the Phase 1 Android build recipe) builds.
- [ ] **Step 5: Commit** `feat(platform): add Android phone adapter`.

### Task 9: Linux call audio

**Files:**
- Create: `crates/carrova-platform/src/linux/call_audio/{mod.rs,bluez.rs,ofono.rs,packagekit.rs,wireplumber.rs}`
- Test: `crates/carrova-platform/tests/linux/call_audio/unit.rs`

**Interfaces:**
- Consumes: Task 7 trait; `linux/screen.rs` `session()` pattern for D-Bus (system bus for BlueZ/oFono/PackageKit).
- Produces (pure, testable): `status_from(adapter: Option<AdapterInfo>, hf_profile: bool, ofono_present: bool, paired_phone: bool) -> CallAudioStatus`; `wireplumber::drop_in(version: &str) -> (PathBuf /* relative to config dir */, &'static str /* contents */)` choosing Lua for `< 0.5`, conf otherwise, contents exactly as recorded by spike S4; `wireplumber::write`/`remove` atomic, idempotent.
- Install: PackageKit `InstallPackages` transaction for `ofono` (OS asks for password); refuse if not Ubuntu ≥ 24.04 (parse `/etc/os-release` `ID`/`VERSION_ID` in `packagekit.rs`; pure fn `supported_os(os_release: &str) -> bool`) with `Unsupported`.
- Pairing: register a BlueZ `Agent1` (`DisplayPasskey` → `on_passkey`), discover, `Pair`, `Trust`; 60 s timeout.
- [ ] **Step 1: Failing tests:** `status_table` (every row of the spec's states), `drop_in_for_04_is_lua`, `drop_in_for_05_is_conf`, `write_then_remove_leaves_no_file`, `install_refused_before_2404`.
- [ ] **Step 2–4:** FAIL → implement → PASS. Manual check on the laptop: `cargo run --example call_audio_status -p carrova-platform` prints the real state (example kept, tiny).
- [ ] **Step 5: Commit** `feat(platform): add Linux Bluetooth call audio`.

---

## Part D — Core

### Task 10: Notify-apps store and filters (phone side)

**Files:**
- Create: `crates/carrova-core/src/ecosystem/phone/{mod.rs,apps.rs,otp.rs}`, `crates/carrova-core/data/private-apps.txt`, `crates/carrova-core/data/otp-words.txt`
- Test: `crates/carrova-core/tests/ecosystem/phone/apps/main.rs`, `…/otp/unit.rs`

**Interfaces:**
- Produces: `pub struct NotifyApps { … }` with `open(platform) -> Result<Self>`, `allowed(&self, app_id: &str) -> bool`, `set(&mut self, platform, app_id, on: bool) -> Result<()>`, `hide_codes()`, `show_text()`, setters; file shape per Global Constraints; new app → on unless listed in `private-apps.txt` (`include_str!`, one package id per line, `#` comments). `pub(crate) fn outgoing(note: Note, apps: &NotifyApps) -> Option<Note>`: `None` if app off; OTP → text `""` and title `""`, `hidden: Some(Code)` (a code can sit in the title too); `show_text` off → text `""`, `hidden: Some(TextOff)`.
- `otp::looks_like_code(text: &str) -> bool`: a 4–8 digit run within 30 chars of any word in `otp-words.txt` (case-insensitive).
- [ ] **Step 1: Failing tests:** `new_app_defaults_on`, `private_app_defaults_off` (e.g. a listed bank id), `file_mode_0600_and_shape`, `otp_detected` / `otp_not_detected_in_prices` (`"Your order total 1299 rupees"` without code words → false), `show_text_off_strips_text`, `otp_clears_title` (title `"Verification code 123456"` → outgoing title without the digits).
- [ ] **Step 2–5:** FAIL → implement → PASS → commit `feat(core): add phone notification filters`.

### Task 11: Phone-side forwarder (giver)

**Files:**
- Create: `crates/carrova-core/src/ecosystem/phone/give.rs`
- Modify: `crates/carrova-core/src/ecosystem/features.rs` (lane dispatch + gate rows for `Notify`/`Messages`/`Calls`), `supervisor.rs` (start `watch_phone_events` when any linked peer has a Phase 2 feature on, stop when none)
- Test: `crates/carrova-core/tests/ecosystem/phone/give/main.rs` (two supervisors, `Recording` as phone)

**Interfaces:**
- Consumes: Tasks 3–7, 10.
- Behaviour: on link open send `Snapshot` (≤ 50) if `notifications` allowed both sides, then `Call(state)` if `calls` allowed; forward each `PhoneEvent` to every linked peer allowing that feature; serve `NotifyAction`, `SmsRequest`, `CallCommand` only if this side's policy allows the feature (else `Refused(NotAllowed)` at the lane gate); `Answer` → `call_answer` then `call_speaker(true)` unless `on_laptop_audio`; `Send` deduped by `(peer, client_id)` for the link session; sms pages respect the "Keep in sync" window (`since_ms`).
- [ ] **Step 1: Failing tests:** `snapshot_on_link_open`, `app_off_not_forwarded`, `phone_policy_off_blocks_even_if_laptop_on`, `reply_reaches_platform`, `reply_to_removed_note_reports_gone`, `answer_turns_speaker_on`, `answer_with_laptop_audio_keeps_speaker_off`, `watch_stops_when_no_feature_on`, `dropped_event_resyncs_every_feature` (after `Dropped`: notification `Snapshot`, current `Call(state)` from `call_state()` — a stale `Active` is replaced by `Idle` — and `SmsPush::Resync`, each only to peers allowing that feature).
- [ ] **Step 2–5:** FAIL → implement → PASS → commit `feat(core): forward phone events over the link`.

### Task 12: Laptop-side mirror (receiver)

**Files:**
- Create: `crates/carrova-core/src/ecosystem/phone/mirror.rs`
- Modify: `crates/carrova-core/src/session/events.rs` (new `Event` variants), `control/wire.rs` + `control/mod.rs` (requests below), `supervisor.rs`
- Test: `crates/carrova-core/tests/ecosystem/phone/mirror/main.rs`, `tests/ecosystem/control.rs` (wire round-trip)

**Interfaces:**
- Produces:
  - `Event::{PhoneNote { device: DeviceId, note: Note }, PhoneNoteGone { device, key }, PhoneSms { device, message: SmsMessage }, PhoneCall { device, state: CallState }, PhoneCleared { device, feature: Feature }}`.
  - `SupervisorHandle::{notes(&DeviceId) -> Result<Vec<Note>>, note_action(&DeviceId, NotifyAction) -> Result<ActionResult>, sms(&DeviceId, SmsRequest) -> Result<SmsReply>, call(&DeviceId, CallCommand) -> Result<CallReply>, phone_settings() / set_phone_settings(PhoneSettings)}` and matching `ControlRequest` variants.
  - `pub struct PhoneSettings { pub ring_here: RingHere, pub only_while_active: bool, pub show_when_locked: bool }`, `pub enum RingHere { Always, WhileActive, Never }` (default `WhileActive`), stored like `ClipSettings` (`clip_settings.rs` pattern).
- Behaviour: in-memory `Mirror` per device; cleared on feature off (either side), link close, forget → `PhoneCleared`. Pop-up coalescing: at most one `PhoneNote` event per app per 2 s, the mirror keeps all. Locked screen (`screen_lock` ≠ `Unlocked`, unless `show_when_locked`) → event carries the `Note` with empty `app_name`/`title`/`text`, no icon, `hidden: Some(Locked)` (count-only). `sms Send` while away → `CoreError` mapped to "away". `SmsPush::Stored { thread_id }` (or 10 s after a provisional `id == 0` message with no `Stored`) → refetch that thread's newest page and replace the provisional row (test `provisional_sms_replaced_after_stored`). `SmsPush::Resync` → drop cached threads/messages and emit `PhoneCleared { feature: Messages }` so open views refetch.
- [ ] **Step 1: Failing tests:** `feature_off_clears_mirror`, `forget_clears_all_phone_data`, `burst_coalesces_per_app`, `locked_screen_hides_content`, `unknown_lock_fails_closed`, `send_twice_same_client_id_sends_once`, `send_while_away_refused`, `control_wire_roundtrip_phase2`.
- [ ] **Step 2–5:** FAIL → implement → PASS → commit `feat(core): mirror phone data on the laptop`.

### Task 13: Calls controller (laptop)

**Files:** Create `crates/carrova-core/src/ecosystem/phone/calls.rs`; Test `crates/carrova-core/tests/ecosystem/phone/calls/main.rs`

**Interfaces:**
- Consumes: Task 12 events/settings, Task 9 platform audio.
- Produces: `pub enum CallView { None, Ringing { device, number, name, ring_here: bool }, Active { device, since_ms, muted, audio: AudioSide }, Missed { device, number, name }, Left { device } }` emitted as `Event::CallView(CallView)` — the one source of truth the ringing window, call window, home bar and tray render. `Tones`/`AudioTo` routed to the local platform; others to the phone.
- `ring_here` = `Always` → true; `Never` → false; `WhileActive` → `Platform::screen_lock() == ScreenLock::Unlocked` (no idle signal exists; lock state is the proxy, `Unknown` rings).
- [ ] **Step 1: Failing tests:** `ringing_then_answer_then_end`, `decline_from_laptop`, `missed_after_ringing_ends_unanswered`, `ringing_clears_when_link_drops` (→ `Left`), `never_setting_does_not_ring_here`, `tones_go_to_local_audio`.
- [ ] **Step 2–5:** FAIL → implement → PASS → commit `feat(core): add calls controller`.

### Task 14: Redaction for phone data

**Files:** Modify `crates/carrova-report/src/redact.rs` (table); Test `crates/carrova-report/tests/redact/…`

- [ ] **Step 1: Failing tests:** `redacts_phone_numbers` (`+91 98765 43210`, `9876543210`, `(415) 555-0100`), `keeps_short_numbers_and_versions` (`v0.2.0`, `port 4433`), `report_from_phone_failure_has_no_content` (format a `CoreError` built from a reply with note text, SMS text, number; assert none appear).
- [ ] **Step 2–5:** FAIL → implement → PASS → commit `fix(report): redact phone numbers`.

---

## Part E — View-models and front-ends

### Task 15: View-models and IPC

**Files:**
- Create: `crates/carrova-view/src/phone.rs`
- Modify: `crates/carrova-view/src/ipc/*` (commands for Task 12 handle methods), `tests/export_bindings/main.rs`; `apps/desktop/src-tauri/src/` thin command wrappers only
- Test: `crates/carrova-view/tests/phone/main.rs`

**Interfaces:**
- Produces (pure functions, `ts` derive): `notes_view(&[Note], locked: bool) -> NotesView`; `threads_view(&[SmsThread], now_ms) -> ThreadsView` (Today/Yesterday/date labels); `thread_view(&[SmsMessage]) -> ThreadView`; `call_view(&CallView, now_ms) -> CallWindowView` (timer text `mm:ss`); `call_log_view(&[CallRecord], now_ms) -> CallLogView` (Today/Yesterday groups); `call_audio_view(CallAudioStatus) -> CallSetupStep` (`Intro | Phone | Install | Pair | Test | Done | Off | NoBt`); `perm_view(PhonePerm, PermState) -> PermStep` (`Ask | Restricted | OpenSettings | Ok`).
- [ ] **Step 1: Failing tests:** `call_timer_format`, `log_groups_today_yesterday`, `locked_notes_show_count_only`, `setup_step_per_status` (table), `restricted_perm_shows_restricted_step`.
- [ ] **Step 2–4:** FAIL → implement → PASS; `just bindings`; `just bindings-check`.
- [ ] **Step 5: Commit** `feat(view): add phone view-models and IPC`.

### Task 16: Desktop notifications and messages

**Files:** Create `apps/desktop/src/screens/phone/{notes/,messages/}`; Modify device page (`apps/desktop/src/screens/devices/`), i18n `apps/desktop/src/i18n/en/ecosystem.ts`; Test `apps/desktop/tests/screens/phone/…`, Playwright `apps/desktop/e2e/phone.spec.ts`

- Screens per mockup: device-page rows (+ `missing` banner "Remind on phone"), Phone notifications (popup, reply, apps sheet read-only from the phone's list, locked, missing), Messages (thread, compose, failed, empty, permission, away). The pop-up is a Carrova window (`WebviewWindow` created by the existing window helper), never OS-notification content; OS notification text is only "<Phone>: new notification".
- [ ] **Step 1: Failing tests** (Vitest per state, fixtures from Task 15 views): `popup_reply_sends_action`, `locked_shows_count_only`, `thread_send_shows_failed_then_retry`, `away_disables_send`.
- [ ] **Step 2–4:** FAIL → implement → PASS (`just ui-test`), Playwright run against the e2e app.
- [ ] **Step 5: Commit** `feat(desktop): add phone notifications and messages`.

### Task 17: Desktop calls and calls setup

**Files:** Create `apps/desktop/src/screens/phone/calls/`, `…/calls-setup/`; Modify home call bar, tray `call` state (`apps/desktop/src-tauri/src/tray.rs` labels only), Settings › Calls and Bluetooth; Test alongside Task 16's.

- Part 1 (base): Incoming call window (ringing, missed; always-on-top, bundled CC0 ringtone), In a call (active, muted, keypad hidden unless audio `Laptop`, moved), Calls (log, empty, notset → for base calls "notset" only when calls feature is off), home bar, tray, Settings ring-here + only-while-active.
- Part 2 (audio): Calls setup wizard states `intro, phone, install, pair, test, done, off, nobt` driven by `CallSetupStep`; install asks before the WirePlumber change; "Set up again" and turning audio off call `set_call_audio_config(false)`.
- [ ] **Step 1: Failing tests:** `ringing_answer_calls_command`, `left_state_closes_window`, `keypad_only_with_laptop_audio`, `wizard_follows_status` (table), `install_needs_agree_before_config`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** (two commits) `feat(desktop): add calls on the laptop`, `feat(desktop): add Bluetooth call audio setup`.

### Task 18: Android screens

**Files:** Modify `apps/desktop/src/mobile/pairing/ChooseShare.tsx`, `apps/desktop/src/mobile/PhonePage.tsx`; Create `apps/desktop/src/mobile/phone/{NotifyApps.tsx,Messages.tsx,Calls.tsx,PermStep.tsx}`; Test `apps/desktop/tests/mobile/…`

- Screens per mockup + Task 2 states: Choose what to share (Phase 2 rows, `sms`, `battery`, `restricted`, `access`, `phoneperms`), device page rows, Notifications on the laptop (apps with search, hide codes, show text, `noaccess`), Messages (synced, first, noperm, rcs; Keep in sync window), Calls (setup, pairing, ready, active, btoff). Permission prompts only when the matching switch turns on, via `PermStep`.
- [ ] **Step 1: Failing tests:** `turning_on_messages_asks_sms_perm`, `restricted_shows_app_info_button`, `private_app_off_by_default_in_list`, `keep_in_sync_default_30_days`.
- [ ] **Step 2–4:** FAIL → implement → PASS; Android debug build installs on emulator `carrova-34` and the four screens render (controller smoke).
- [ ] **Step 5: Commit** `feat(android): add phone feature screens`.

---

## Part F — Finish

### Task 19: Docs, checklist, CI

- [ ] **Step 1:** `docs/PLAN.md` Ecosystem table: Phase 2 "Implemented; exit gate pending"; status line; `CLAUDE.md` Status; spec Status → implemented.
- [ ] **Step 2:** `tasks/todo.md`: section "Ecosystem Phase 2 — exit gate" with the spec's real-hardware lines as checkboxes (+ "Android emulator smoke (controller)").
- [ ] **Step 3:** `boundary-reviewer` over the branch; fix findings (own commits).
- [ ] **Step 4:** `just ci` in the capped unit and `just e2e --link`; show the tail of both logs (`CI EXIT 0`, `PASS`).
- [ ] **Step 5: Commit** `docs: mark ecosystem phase 2 implemented`. Then `cargo clean` if `target/` > ~20 GiB. Ask the user before push/PR.
