# ADR 0005: Screens and input over the link

- Status: Proposed (awaiting user sign-off). Open questions 1–5 (builder
  check, 2026-11-20) block sign-off. Phase 2's hardware exit gate is still
  pending, so no Phase 3 code starts before it passes even once this is
  signed.
- Date: 2026-11-20
- Scope: Ecosystem Phase 3, "Gate 0"
  (`docs/superpowers/specs/2026-11-20-ecosystem-phase3-design.md`).
- Builds on: ADR 0002 (link intent, proofs, one connection per pair),
  ADR 0003 (Android bridge, the one `unsafe` module).
- Touches: the link's lane set (three new lanes), one transport API call
  (stream priority), the input-injection trust model, OS permissions
  (portals, Android `mediaProjection` service type, an accessibility
  service). **No** `carrova-crypto` change and **no** handshake change
  (`Hello`, intents, handshake frames, close codes, `PROTOCOL_VERSION`
  unchanged). Signed off anyway because screen and input are the most
  sensitive things Carrova will carry (CLAUDE.md "Crypto"; parent design
  "ADR if screen or input streams need crypto/handshake changes").

## Context

Phase 3 lets a linked device see another's screen, drive it, use a phone
as touchpad and keyboard, and share one keyboard and mouse between two
computers. The question for this ADR: does that need anything beyond the
existing authenticated link, and what must hold so injected input is safe.

What exists:

- One QUIC connection per linked pair, TLS 1.3 (quinn 0.11, rustls +
  `ring`), established only after the ADR 0002 link proofs
  (`KnownSecret` HMAC and `DeviceLink` signature, both bound to the TLS
  channel). Everything on it is encrypted and authenticated to the peer's
  device key.
- Lanes are bidirectional QUIC streams opened with `LaneOpen { kind,
  version }`; the acceptor checks a local `LaneGate` before reading past
  the open frame and refuses unknown kinds/versions without dropping the
  connection (`crates/carrova-link/src/lane.rs:24-79`, `117-157`).
- Lane frames are length + CBOR capped at `MAX_FRAME_LEN` = 1 MiB; bulk
  data goes as raw bytes after a header frame, as the `File` lane does
  (`crates/carrova-transport/src/lib.rs:125`,
  `crates/carrova-transport/src/stream.rs:33-77`,
  `crates/carrova-link/src/file.rs:3-10`, `102-115`).
- Transport: 4 MiB stream / 32 MiB connection windows, 16 bidirectional
  streams, unidirectional streams disabled, no datagrams, no stream
  priorities set (`crates/carrova-transport/src/endpoint.rs:55-68`,
  `tuning.rs:28-35`).
- `Feature::{Screen, Input}` are reserved and read `Off`
  (`crates/carrova-link/src/policy.rs:30-33`, `81`); the core gate maps
  lanes to features (`crates/carrova-core/src/ecosystem/features.rs:
  116-137`).
- `Platform::screen_lock()` reports lock state, `Unknown` fails closed
  (`crates/carrova-platform/src/platform.rs:411-416`).
- Android: Rust → Kotlin calls and Kotlin → Rust JSON events through
  `android/jni.rs`; an fd detached from a `ParcelFileDescriptor` becomes a
  Rust `File` (`jni.rs:101-111`). `LinkService` is a `connectedDevice`
  foreground service.

## Decision

### 1. No new connection, handshake or crypto

Screen sessions, video and input run as lanes on the existing link
connection. They inherit its encryption (TLS 1.3 AEAD), integrity, replay
protection and peer authentication (ADR 0002). No per-session keys, no
SRTP/DTLS, no second connection, no relay. `carrova-crypto` and the
handshake are untouched. A session cannot outlive or move to another
connection (§3).

### 2. Three new lanes

| Kind | Opener | Carries | Gate (target's local policy) |
|---|---|---|---|
| `Screen` | the side starting the session | `Request`, `Answer`, pause/resume, control ask/grant/take, stop, keyframe, feedback, heartbeat, keyboard-and-mouse `Leave` | admitted if the local `screen` or `input` for this peer is not `off` (the `Request` kind is then checked against the right one), or if it is a push offer (always asked) |
| `Video` | the sharer | `Config`, then `VideoFrame { seq, pts_us, key, len }` + `len` raw bytes (`len` ≤ 4 MiB) | admitted only with a live session id (§3) |
| `Input` | the controller | batches of ≤ 64 input events, ≤ 4 KiB; target → controller `InputAck { seq }` per batch | admitted only with a live, control-approved session id (§3) |

- All `version: 1`. Older peers refuse them `Unknown`; the connection
  stays up (`lane.rs:81-115`). No change to `LaneReply`; `LaneOpen`
  changes only if open question 1 picks (a) (optional `session`, §3).
- Frame headers stay within `MAX_FRAME_LEN`; video payload bytes are raw
  after the header, like file chunks. A video frame is not hashed (TLS
  already authenticates every byte; a hash adds cost for nothing here).
- The session ends when its `Screen` lane ends from either side or drops;
  the target then refuses and closes that session's `Video`/`Input` lanes.

### 3. Session binding

- *Pending open question 1 (recommended (a)):* `LaneOpen` gains an
  optional `#[serde(default)] session: Option<[u8; 16]>` and
  `LaneGate::admit` takes `(kind, session)`, because today the gate sees
  only the kind before anything is read (`lane.rs:76-79`, `133-157`).
  Older peers ignore the field. The `Screen` lane sends none.
- On approval the target draws 16 random bytes (`SessionId`, OS RNG via
  `getrandom`, already a `carrova-core` dependency; no new primitive) and
  returns it in `Answer::Accept`. The `Video` and `Input` lanes carry it
  in `LaneOpen` (open question 1).
- The side receiving such a lane admits it only if a session with that
  id is live, of the right kind, approved for control (for `Input`),
  **belongs to the same `Connection`**, and the opener holds that lane's
  role in it (the sharer for `Video`, the controller for `Input`). This
  covers the viewer too: in a view request the target opens `Video`, so
  the viewer checks it against the session it requested. The id is not a secret against the peer (the
  peer is authenticated); it binds lanes to one approval and one
  connection, so nothing approved on one connection is usable on another
  and a stale lane cannot reattach to a newer session.
- Concurrency per device: one shared-screen, one control, one
  keyboard-and-mouse session; more are refused `Busy`.

### 4. Input-injection trust model

A peer's input reaches the OS only when all hold:

1. The peer is linked and on its authenticated link connection (ADR 0002).
2. The target's local `input` for that peer is `ask` (never `on`: there is
   no "always allow control"), and the global "Allow control requests" is
   on.
3. A person on the target said yes to this session (60 s ask, never
   remembered); for control inside a view, a second yes after the view's.
4. The session is live, on this connection (§3), not paused, and the
   target is unlocked (`screen_lock() == Unlocked`; `Unknown` = locked).
5. The desktop app (indicator surface) is running; the headless
   supervisor refuses. *Builder call (main thread), user may revisit:*
   an explicit `SupervisorConfig` flag (`supervisor.rs:43-55`), off when
   `carrova link serve` builds it, with a test.
6. The OS gate itself passed: Wayland RemoteDesktop/InputCapture portal
   session, Android accessibility service enabled by the user, macOS
   Accessibility/Input Monitoring (later), Windows UIPI limits (later).

Limits and safety:

- ≤ 2000 events/s averaged over 1 s, ≤ 64 events and 4 KiB per batch,
  `Text` ≤ 256 bytes, coordinates clamped to the shared surface,
  `Gesture` ≤ 64 points. Exceeding a rate limit or sending a malformed
  event ends the session (`Stop { Protocol }`).
- The target releases every key and button it pressed on any exit (stop,
  pause, lane drop, link loss, rate-limit end, forget, policy change) and
  when the controller is lost: no message on the `Screen` lane for 2 s.
  The 500 ms `Heartbeat` there is the held-input keepalive, so a healthy
  session holding a key (no new input events) keeps it held.
- Every session, control included, ends with its connection (§1, §3).
  Control and touchpad never come back without a new yes. *Pending open
  question 4 (recommended):* no approval is carried to a new connection.
  Every keyboard-and-mouse session needs a fresh yes on the target too;
  only "Always allow viewing" starts a view-only session without an ask
  (indicator again), as it does on first connect.
- *Pending open question 3 (recommended):* input only while the target is
  unlocked, always; the mockup toggle "Only when the other computer is
  unlocked" is dropped.
- Forget, `input` → `off`, "Allow control requests" off, or logout end
  every input session at once.
- Android: the accessibility service subscribes to no accessibility
  events and reads no window content; it dispatches gestures, global
  actions and text only while a control session is live.

### 5. Viewing trust model

- `screen` states `ask` / `on` / `off` (new `Group::View`); `on` =
  "Always allow viewing": skips Carrova's ask for **view only**; the
  indicator still shows and OS prompts (portal, MediaProjection) still
  apply. Only the target's own setting gates (directional, unlike Phase
  1's two-sided `allowed()`); a push share always asks the viewer.
  *Builder call (main thread), user may revisit:* the initiator's own
  setting never blocks starting a session; `Supervisor::live()`
  (`crates/carrova-core/src/ecosystem/supervisor.rs:191-207`) checks the
  target's value for `screen`/`input` instead of `allowed(local, remote)`.
  Which viewer setting admits a push is open question 2 (recommended: the
  viewer's per-device `screen` not `off`, read as "screen sharing with
  this device").
- The sharer pauses frames while locked (`Paused { Locked }`) and on
  resume needs no new ask.
- The indicator is always on (spec "Indicator"); on GNOME Wayland the
  system's portal indicator is the guaranteed surface.

### 6. Transport changes (API only)

- `Stream::set_priority(i32)` wrapping quinn's per-stream send priority.
  Order: `Input` > `Screen` > `Video` > other lanes, so a file transfer or
  a keyframe never delays a keystroke. No transport parameter or
  handshake change.
- The sender of a `Video` lane keeps at most two encoded frames unacked
  (drop-to-latest at capture; every encoded frame is sent, so the
  reference chain stays whole). Two in flight keep 30 fps while RTT +
  decode + ack stays under about 66 ms. *Pending open question 5 (recommended (b)):* the
  viewer acks each frame `seq` on the `Screen` lane, because `Stream` has
  no split (`crates/carrova-transport/src/stream.rs:23-88`) and the 4 MiB
  per-stream window (`tuning.rs:28`) holds about 8 s of video, so write
  blocking cannot bound the queue. A decode failure is answered with
  `Keyframe { failed: seq }` instead of an ack: it releases the credit and
  the next encoded frame is a keyframe. Flow-control windows stay as they
  are; no `Stream::split`.
- Datagrams and unidirectional streams stay off. Turning on datagrams
  later would negotiate a transport parameter and needs a new ADR.

### 7. OS permissions added

- Linux: xdg-desktop-portal `ScreenCast` (view), `RemoteDesktop` with
  `ConnectToEIS` (control, touchpad, keyboard-and-mouse target),
  `InputCapture` (keyboard-and-mouse source). A portal restore token, if
  granted by "Remember this selection", is stored at
  `<state>/screen/portal-token` (`0600`), deleted on logout-scoped expiry
  by the portal and by Carrova when no device has `screen` on.
- Android manifest: `FOREGROUND_SERVICE_MEDIA_PROJECTION`;
  `LinkService` adds the `mediaProjection` type for a session's length;
  an `AccessibilityService` (`BIND_ACCESSIBILITY_SERVICE`,
  `canPerformGestures`, `flagInputMethodEditor`, no event types, no
  `canRetrieveWindowContent`). Enabled only by the user in Android
  settings (sideloaded: "Allow restricted settings" first).
- No new `unsafe`: capture, encode and injection go through safe-API
  crates; Android video bytes reuse `file_from_fd` (`jni.rs:101-111`).
  ADR 0003 unchanged.

### 8. Codec and patents (recorded here because it is a distribution decision)

The viewer advertises decodable codecs; the sharer picks `h264` only from
an OS/device encoder, else `vp8` (libvpx, BSD, royalty-free). Carrova
never ships an H.264 encoder.

## Alternatives considered

| Option | Why not |
|---|---|
| WebRTC (DTLS-SRTP, ICE) for media | A second key exchange and connection beside the link, with its own identity binding to prove; large stack; LAN only needs what QUIC already gives |
| Per-session media keys inside the link | TLS 1.3 already encrypts and authenticates every byte to the linked key; extra keys add code and nothing against any listed threat |
| QUIC datagrams for video | No head-of-line blocking, but needs FEC/NACK and a negotiated transport parameter; deferred until a measured need |
| Video inside the `Screen` lane | One lane fewer, but control messages and feedback wait behind keyframes; separate lanes let priority work |
| Session id as a bearer token across connections | Would let a session follow a reconnect without the target noticing; binding to the connection is simpler and stricter |
| "Always allow control" | Unattended access is an anti-goal; the mockups state "there's no Always allow" |
| Bump `PROTOCOL_VERSION` | Nothing in the handshake changes; lanes are already versioned and refused gracefully |

## Consequences

- `carrova-link`: `LaneKind::{Screen, Video, Input}`, their messages,
  `Group::View`, `Feature::PHASE3`, `screen`/`input` leave "reserved".
- `carrova-transport`: `Stream::set_priority`. Nothing else.
- `carrova-crypto`: unchanged.
- `carrova-core`: session table keyed by (connection, `SessionId`);
  directional gate for `screen`/`input`; stuck-key release; lock pause.
- `carrova-platform`: capture, injection and input-capture traits;
  Android manifest and glue as §7.
- An older peer cannot share or view screens; everything else on the link
  keeps working.

## Threat model

Additions to ADR 0002's table.

| Threat | Mitigation | Residual risk |
|---|---|---|
| **Stranger on the Wi-Fi requests a view or injects input** | No link connection without ADR 0002 proofs; lanes exist only on it | None beyond ADR 0002 |
| **Stolen phone (still linked)** | View and control ask on the target every time; `input` defaults `off`; "Always allow viewing" is opt-in and still shows the indicator | With "Always allow viewing" set, a thief sees the screen until the device is forgotten; the bar and GNOME's indicator show it |
| **Peer injects input without/after approval** | `Input` lanes need a live, control-approved session id on the same connection; checked per lane open; session end closes lanes | None known |
| **Approval replayed on a new connection** | Session bound to its `Connection`; control never resumes on reconnect | With "Always allow viewing", a view-only session starts again after a reconnect without an ask; it needs the same key's fresh proofs and shows the indicator |
| **Input flood / malformed events** | Rate and size caps, clamping; violation ends the session | Up to 2000 events in the second before the cut |
| **Stuck keys after a crash or drop** | Target-side release on every exit and on 2 s without `Screen` lane messages (heartbeat lost) | A target process crash leaves the OS to release (portal/EIS sessions close with the process) |
| **Typed secret crosses to the other computer** | No crossing while a password field is focused (best effort per OS), while locked, or pinned | Linux detection depends on AT-SPI; lock screen always blocks |
| **Hidden sharing** | Indicator always on; no session without the desktop app; GNOME's own indicator for portal sessions; Android system chip + notification | On GNOME Wayland Carrova's own bar may be covered by other windows |
| **Sensitive windows captured** | OS protected-content flags honoured on Windows, macOS, Android; pause on lock | Linux has no protected-content mechanism: everything visible is shared (stated to the user) |
| **Screen data leaks into logs/reports** | Frames, keys, text, positions never logged; redaction test | None known |
| **Codec bugs on malicious frames** | Decoding happens in the webview's sandboxed media stack, frames come only from an authenticated peer | A malicious linked peer can feed crafted frames to the system decoder |
| **Accessibility service abuse** | Service inert outside sessions, no event subscription, no window content; user enables it explicitly | A compromised Carrova app could use it; same as any accessibility app |

## Storage

- `<state>/screen/portal-token` (`0600`): portal restore token, optional.
- `<state>/kvm/layout.json` (`0600`): screen arrangement (spec).
- Policy keys `screen`, `input` in the existing per-device policy file.
- Nothing about a session (ids, frames, events) is persisted.

## Logging

ADR 0001/0002 lists apply. Additionally never logged: frames or their
hashes, typed text, key or HID codes, pointer positions, gesture paths,
window titles, session ids, the portal restore token. Allowed: codec,
size, bitrate, fps, counts, durations, end reason.

## Open questions for the user

Builder check (2026-11-20), blocking sign-off; full text and options in
the spec's open questions 1–5:

1. **Session id at the lane gate**: (a) optional `session` on `LaneOpen`,
   (b) gate reads one more frame, (c) accept then check and close.
   *Recommend (a).*
2. **Push-share gate**: viewer's per-device `screen` not `off` ("screen
   sharing with this device"), push always asks. *Recommend this.*
3. **Locked target**: input only while unlocked, always; drop the mockup
   toggle. *Recommend this.*
4. **Reconnect**: sessions end with their connection; nothing carries
   over, so every keyboard-and-mouse (and control, touchpad) session
   needs a fresh yes. A 30 s view-only grace would conflict with the
   anti-goal "no screen seen without a fresh yes" unless "Always allow
   viewing" is on, which already skips the ask. *Recommend this.*
5. **Transport scope**: frame acks on the `Screen` lane instead of
   `Stream::split` + backlog signal or a smaller `Video` window.
   *Recommend acks: no transport change beyond `set_priority`.*

Original:

6. **No crypto or handshake change**: screens and input ride the link's
   TLS 1.3 with no extra keys. Agree?
7. **Keyboard-and-mouse approval survives a reconnect**: superseded by
   question 4 (no carried approval).
8. **Directional gate** for `screen`/`input` (only the target's setting
   counts). Agree?
9. **Stream priority** (`Input` > `Screen` > `Video` > files) as the only
   transport change; datagrams deferred. Agree?
10. **Android accessibility service** with gestures and the input-method
   flag, inert outside sessions; APK-only distribution (Play review of
   accessibility use is a later decision). Agree?
11. **Never ship an H.264 encoder**; VP8 baseline, H.264 from OS encoders
   only. Agree?
