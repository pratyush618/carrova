# Ecosystem Phase 3: screens and input

Date: 2026-11-20. Status: proposed (awaiting user sign-off, with ADR 0005).
Parent design (decisions, approaches, roadmap):
`docs/superpowers/specs/2026-11-15-ecosystem-design.md`. Builds on Phase 1
(`2026-11-15-ecosystem-phase1-design.md`) and Phase 2
(`2026-11-16-ecosystem-phase2-design.md`). Transport and trust decisions:
`docs/adr/0005-screens-and-input.md`. Screens:
`docs/mockups/gui-ecosystem-desktop.html` (`screen`, `viewer`, `kvm`, `peer`,
`phone`, `settings:screen`, `tray:sharing|controlled`) and
`docs/mockups/gui-ecosystem-android.html` (`share-screen`, `viewer`,
`control`, `touchpad`, `device:screen`, `settings:screen`,
`sys:sharing|controlled`).

This document narrows the parent design to Phase 3 and pins what the
parent left open for it. Where they differ, this one wins for Phase 3.
Phase 1 and 2 pinned values (link, lanes, policy file, asks, Android
shell, bridge) hold unless changed here.

## Why

The parent design promises "one place for everything between my devices".
Phases 1 and 2 moved data (clipboard, files, notifications, texts, calls).
What is left is the thing people reach for TeamViewer, Barrier/Input Leap
and scrcpy for: see another screen, drive it, and use one keyboard and
mouse across two computers. Doing it over the link the user already trusts
means no account, no relay, no second pairing, and the same
ask-on-the-target rules as files.

## What Phase 3 delivers

A Linux (GNOME/Wayland, X11 fallback) laptop linked to another Linux
computer and an Android 13+ phone, on which the user can:

1. **View** another device's screen: computer ↔ computer, phone → computer
   (mirror), computer → phone (phone views the laptop). View only.
2. **Control** it from the viewer after a second, separate yes: computer ↔
   computer, phone drives the laptop, laptop drives the phone (Android
   accessibility service).
3. **Use the phone as touchpad and keyboard** for a computer.
4. **Share one keyboard and mouse** between two computers: arrange
   screens, move the pointer off an edge, type there, come back.

Every session is asked on the device being seen or driven, shows an
indicator there that cannot be turned off, pauses while that device is
locked, and ends cleanly.

## Goals

- Latency good enough to work, not to game (budgets below).
- Readable text at 1080p on a LAN; graceful drop in quality, not freezes,
  on bad Wi-Fi ("Lower quality · slow network").
- Zero cost while idle: no capture, no encoder, no input hook runs unless a
  session is live.
- Same trust model as the rest of the ecosystem: linked devices only,
  target asks, policy per device, never changed remotely.

## Non-goals (v1)

- Internet relay or use away from the LAN (parent non-goal).
- **Audio** (system sound with the picture). Not cheap: per-OS loopback
  capture, Android `AudioPlaybackCapture` limits, A/V sync. Later.
- **File drag across screens** in keyboard-and-mouse. Use Send files.
- More than one viewer per shared screen at a time; viewing several
  monitors at once (one source per session; the OS picker chooses).
- Unattended access (any session without a person saying yes on the
  target). Never, not just not in v1.
- Recording, annotation, chat, remote reboot, login/lock-screen control,
  UAC/elevated windows.
- HDR, 4K60, gaming latency.
- Windows and macOS adapters beyond compiling (methods return
  `Unsupported`; UI says "This computer can't share its screen yet"). The
  mechanisms are named below so the follow-up is planned, but they do not
  block the Phase 3 exit gate (parent: adapters follow each phase).
- Headless sharing: `carrova link serve` refuses screen and input sessions
  (there is no indicator to show).

## Anti-goals (failure even if it works)

- A screen is seen or a device driven without a fresh yes on that device
  (control, touchpad and keyboard-and-mouse) or, for view-only with
  "Always allow viewing", without the indicator showing.
- The indicator can be hidden, turned off, or outlives the session in a
  misleading way (shows nothing while something is shared).
- A stranger, an unlinked device or a forgotten device can request, see or
  inject anything.
- Input keeps landing after Stop, lock, forget, link loss or a policy
  change to Never.
- A key stays pressed on the target after a session ends.
- Frames, keystrokes, typed text or pointer positions end up in logs,
  crash reports or error reports.
- A password typed on one computer lands on the other because the pointer
  crossed.

## Constraints

- CLAUDE.md holds: OS code only in `carrova-platform`; `src-tauri` no
  business logic; dependencies point down; `thiserror`; tests outside
  `src/`; nothing secret logged; `just ci` green.
- Crypto/handshake changes need sign-off: ADR 0005 is written first. It
  finds **no** `carrova-crypto` change and **no** handshake change (see
  "Link additions"), but it pins new lanes, a transport priority call,
  the input trust model and new OS permissions, so it is still signed off
  before code.
- `unsafe_code`: the workspace forbids it; only `android/jni.rs` is
  allow-listed (ADR 0003, `crates/carrova-platform/Cargo.toml` `[lints]`).
  Phase 3 adds **no** new `unsafe`: capture, encode and injection go
  through maintained safe-API crates (they may use `unsafe` inside), and
  Android video bytes reuse the existing fd hand-off
  (`android/jni.rs:101-111`, `file_from_fd`).
- Codecs: audited, maintained crates; no H.264 encoder shipped by us
  (patents; see "Media pipeline").
- Linux Wayland: capture and input only through xdg-desktop-portal; the
  OS prompt is part of the flow (parent constraint; mockup `screen:portal`,
  `kvm:portal`).

## What exists (recon)

- **Lanes.** `LaneKind` enum, `LaneOpen { kind, version: 1 }`, refused
  with `Unknown | NotAllowed | Busy`; the acceptor checks a `LaneGate`
  before reading past the open frame
  (`crates/carrova-link/src/lane.rs:24-44`, `56-64`, `76-79`, `133-157`).
  Unknown kinds/versions from newer peers decode leniently and are refused,
  connection stays up (`lane.rs:81-115`).
- **Streaming example.** `File` lane: CBOR header frame then raw bytes,
  BLAKE3 per chunk, explicit `Done` because a dropped lane looks finished
  (`crates/carrova-link/src/file.rs:3-10`, `76-81`, `102-115`).
- **Stream API.** `Stream::send/recv` (length + CBOR, capped by
  `MAX_FRAME_LEN` = 1 MiB), `write_all/read_exact` raw, blocking with
  timeouts, cancel-aware (`crates/carrova-transport/src/stream.rs:23-77`,
  `crates/carrova-transport/src/lib.rs:125`).
- **Link connection.** quinn 0.11 over rustls/ring, TLS 1.3; one link
  connection per pair, authenticated by ADR 0002 proofs bound to the TLS
  channel. Transport config: 4 MiB per-stream and 32 MiB connection
  windows, 16 bidirectional streams, **no** unidirectional streams, no
  datagrams used, keep-alive per platform (`crates/carrova-transport/src/
  endpoint.rs:50-68`, `tuning.rs:28-44`).
- **Policy.** `Feature::{Screen, Input}` exist as reserved (read `Off`);
  groups `Quiet` (on/off) and `Ask` (ask/off); `allowed()` = off if either
  side off, ask beats on (`crates/carrova-link/src/policy.rs:15-34`,
  `75-83`, `175-191`). New phones get `phone_defaults`, computers
  `computer_defaults` (all off) (`policy.rs:127-146`,
  `crates/carrova-core/src/ecosystem/pair.rs:235`, `supervisor.rs:199`).
- **Gate.** `carrova-core` maps each `LaneKind` to the local feature
  (`crates/carrova-core/src/ecosystem/features.rs:116-137`).
- **Asks.** One ask per offer, 60 s expiry, never remembered
  (`crates/carrova-core/src/ecosystem/asks.rs:1-4`).
- **Platform.** `Platform::screen_lock()` (GNOME/freedesktop ScreenSaver;
  `Unknown` → callers fail closed) (`crates/carrova-platform/src/
  platform.rs:414`, `linux/screen.rs:1-4`). Linux already depends on
  `zbus` and `x11rb` (`crates/carrova-platform/Cargo.toml`). No capture or
  injection code anywhere. Windows and macOS adapters hold fs, roots,
  background, file manager only (`src/windows/`, `src/macos/`).
- **Android bridge.** Rust → Kotlin static calls through `jni.rs`;
  Kotlin → Rust events by blocking pull of JSON strings (`platform/
  Events.kt`, Phase 2 S1); an fd detached from a `ParcelFileDescriptor`
  becomes a Rust `File` (`jni.rs:101-111`). Glue in `apps/desktop/
  src-tauri/gen/android/app/src/main/java/org/byteveda/carrova/platform/`.
  `LinkService` is a `connectedDevice` foreground service.

## Gate 0 (before any Phase 3 code)

1. **Phase 2 exit gate passed** (CLAUDE.md "stay in the current phase").
   Still pending (user's real-hardware checklist, 2026-11-20): the plan
   can be written, but no Phase 3 code starts until it passes.
2. **ADR 0005 signed off** by the user.
3. **Open questions below answered**; answers recorded here.
4. **Spikes** (results written into "Spike results" before the sub-phase
   that needs them):
   - S1 (3a) Ubuntu 24.04 GNOME Wayland: ScreenCast portal + PipeWire into
     Rust (`ashpd` + `pipewire` crates): frame format (SHM vs DMA-BUF),
     damage info, restore token ("Remember this selection"), GNOME's own
     sharing indicator and its stop button, what the stream shows while
     locked. X11: capture by `x11rb` SHM `GetImage` at 30 fps.
   - S2 (3a) WebCodecs `VideoDecoder` in each webview with default
     packages: WebKitGTK on Ubuntu 24.04 (VP8, H.264), Android System
     WebView; decode→canvas latency at 1080p30.
   - S3 (3a) libvpx VP8 realtime encode, 1080p30, on the reference laptop:
     CPU per frame, quality of text; build for AppImage and `.deb`.
   - S4 (3a) Android: MediaProjection started from `LinkService` (adding
     the `mediaProjection` service type at runtime), MediaCodec VP8 and
     H.264 encode into a pipe fd read by Rust; Android 14 "share one app";
     lock behaviour; Android 13, 14, 15.
   - S5 (3b) RemoteDesktop portal + `ConnectToEIS` with the `reis` crate
     on GNOME 46; restore token; XTest on X11; Android accessibility
     service `dispatchGesture` and `AccessibilityService.InputMethod`
     (API 33) on a sideloaded APK (restricted settings).
   - S6 (3d) InputCapture portal on GNOME 46: pointer barriers, release,
     how often GNOME asks (Deskflow reports the dialog on every start:
     github.com/deskflow/deskflow/issues/8032), keys delivered while
     captured, whether GlobalShortcuts is available.

## Sub-phases (recommended order)

Each ends with `just ci` green, its automated tests, and its own
real-hardware check; the Phase 3 exit gate is the sum.

| # | Delivers | Needs | Exit check (user, real hardware) |
|---|---|---|---|
| **3a View** | `Screen` + `Video` lanes, asks, indicator, pause on lock, end summary, codec negotiation, adaptive quality. Computer ↔ computer (Wayland portal, X11 fallback), phone → computer mirror, computer → phone viewer. Desktop `screen`, `viewer` (all but control states), tray `sharing`; Android `share-screen`, `viewer` (view states), `sys:sharing` | S1–S4 | View the second computer for 10 min on GNOME Wayland; mirror the phone; view the laptop from the phone; lock each sharer and see it pause |
| **3b Control** | `Input` lane, control ask, take back, revoke; computer ↔ computer (RemoteDesktop portal/libei, XTest), phone drives laptop, laptop drives phone (accessibility). Desktop `viewer:asking\|control\|revoked`, `screen:control`, tray `controlled`; Android `control`, `viewer:control`, `sys:controlled` | 3a, S5 | Control the second computer: open an app, type a sentence; control the phone: open an app, type; take back control on each |
| **3c Touchpad** | Phone as touchpad and keyboard (no video; reuses 3b's input lane and computer injection). Android `touchpad`, desktop `kvm:touchpad` | 3b | Move, click, scroll, type with modifiers from the phone; laptop sleeps → "went away" |
| **3d Keyboard and mouse** | Arrangement, edge crossing, return hotkey, pin, secure-input block, clipboard along, peer gone. Desktop `kvm` (all states), settings rows | 3b, S6 | Arrange two computers, cross both ways, type on both, pin, password box blocks crossing, unplug the other's network → pointer home |

Why this order: view is the base every other part reuses (session lane,
asks, indicator, lock pause). Control adds only the input lane and
injection. Touchpad is small once injection exists. Keyboard-and-mouse is
the riskiest (InputCapture portal is young) and goes last so it cannot
hold the rest back; if S6 fails, 3d moves to its own plan and the exit
gate's crossing check is dropped (as Phase 2 did with call audio).

## Approaches considered

### Media pipeline

| Approach | Trade-off | Verdict |
|---|---|---|
| **A. OS capture + encode in Rust, frames on a link lane, decode in the webview (WebCodecs)** | Capture per OS behind `Platform`; software VP8 (libvpx) on computers as the baseline (phones use MediaCodec), OS hardware H.264 (MediaCodec now; Media Foundation, VideoToolbox, VA-API later) where available; encoded frames cross Tauri IPC as raw bytes (a few Mbit/s, cheap), the webview decodes in hardware via WebCodecs. One transport, one trust model, no new crypto | **Picked** |
| B. GStreamer pipeline everywhere (pipewiresrc → vp8enc/vah264enc; appsrc on the viewer) | Mature on Linux, and WebKitGTK already ships GStreamer there. Heavy to package on Windows, macOS and Android; a second media stack beside the webview's; C plugin graph is hard to test in-process | Rejected for v1; keep as Linux encode fallback if S3 fails |
| C. WebRTC stack (libwebrtc / webrtc-rs) | Best congestion control, jitter buffer and FEC. Brings its own ICE, DTLS-SRTP keys and connection: a second crypto and transport path beside the link, needing its own trust binding and sign-off; large dependency | Rejected |
| D. VNC-style tiles (changed rectangles, zlib) | No codec patents, crisp text, trivial. Poor at scrolling and video; 1080p full-screen changes cost tens of Mbit/s | Rejected |

**Codec (pinned, Q7).** The viewer advertises what its webview can decode
(`VideoDecoder.isConfigSupported`); the sharer picks the first it can
encode from `[h264 (OS encoder only), vp8]`.

- **VP8** (libvpx, BSD-licensed, royalty-free; Google's patent grant):
  baseline on every computer, realtime mode. Android's CDD requires a VP8
  encoder, so phones can always send it from MediaCodec.
- **H.264** only from an encoder the OS or device ships (Android
  MediaCodec in 3a; Media Foundation, VideoToolbox, VA-API later). The
  device maker/OS carries the patent licence. **We never ship an H.264
  encoder** (no openh264 built from source: Cisco's licence covers only
  its own downloaded binary, openh264.org/faq.html).
- Decoding is the webview's (system codecs). WebKitGTK decodes through the
  system GStreamer, so H.264 there depends on the user's installed plugins;
  VP8 comes with Ubuntu's default `gstreamer1.0-plugins-good`. S2 confirms.
- No Rust-side decoder in v1. If S2 shows WebCodecs missing in a target
  webview, the fallback is decode in Rust (libvpx) and draw RGBA via the
  IPC channel; that is a plan change, written up after S2.

### Media transport (Q5, Q6)

| Approach | Trade-off | Verdict |
|---|---|---|
| **One `Video` lane (QUIC stream) per session; sender keeps at most one frame queued and drops the rest** | No transport change beyond a priority call; reliable, ordered, already encrypted; head-of-line delay on a lost packet is about one LAN round trip (a few ms) | **Picked** |
| QUIC datagrams (RFC 9221) per frame fragment | No head-of-line blocking; needs FEC or NACKs, a new `Connection` API and a negotiated transport parameter (handshake-level, sign-off) | Later, only if 3a's real-hardware check shows stalls |
| One unidirectional stream per frame (reset late ones) | No HOL across frames; needs unidirectional streams enabled (`max_concurrent_uni_streams(0)` today, `endpoint.rs:61`) and per-frame stream churn | Rejected for v1 |

### Keyboard and mouse

| Approach | Trade-off | Verdict |
|---|---|---|
| **Source/target edge crossing**: the computer with the physical keyboard captures its own input when the pointer meets an arranged edge, sends events to the target, which injects them; the target reports when its pointer leaves back across the entry edge | The generic model all software KVMs use; works with the OS input APIs we need anyway (InputCapture/RemoteDesktop portals, XInput2/XTest, low-level hooks/SendInput, CGEventTap/CGEvent) | **Picked** |
| Remote control through the viewer only (no KVM) | Already in 3b; no capture on the source. Needs the picture, adds video latency, no "one desk" feel | Kept as 3b; not a substitute |
| Speak Input Leap/Deskflow's protocol | Interop, but their own unauthenticated/TLS-pinning trust model and protocol, outside the link; parent rejects foreign protocols for trust reasons | Rejected |

## Link additions (pinned; details and threat model in ADR 0005)

| Item | Value |
|---|---|
| New lane kinds | `Screen` (session control), `Video` (frames, sharer → viewer), `Input` (events, controller → target); all `version: 1` |
| Handshake, `Hello`, intents, close codes | unchanged; sessions run on the existing link connection |
| Encryption | the link's QUIC TLS 1.3 (rustls/ring); no new keys, no `carrova-crypto` change |
| Session | one `Screen` lane per session for its whole life; its end (either side, or lane drop) ends the session and both its `Video`/`Input` lanes |
| Session id | 16 random bytes chosen by the target on approval; `Video` and `Input` lanes open with it and are refused (`NotAllowed`) unless a live, approved session with that id exists **on the same connection** |
| Concurrency | per device at most one shared-screen session, one control session and one keyboard-and-mouse session; extra requests refused `Busy` |
| Frame header | `VideoFrame { seq, pts_us, key, len }` CBOR, then `len` raw bytes; `len` ≤ 4 MiB (control frames keep the 1 MiB `MAX_FRAME_LEN`); `Config { codec, width, height, description? }` before the first frame and on change |
| Input frames | batches of ≤ 64 events, ≤ 4 KiB; target accepts ≤ 2000 events/s averaged over 1 s, ends the session above it |
| Priority | `Input` > `Screen` > `Video` > everything else (files) through quinn's per-stream send priority (new `Stream::set_priority`, transport API only) |
| Old peers | an older Carrova refuses the new lanes `Unknown`; UI: "Update Carrova on <device> to share screens" |

### `Screen` lane messages

- Opener → target: `Request { kind: View | Control | Kvm | Touchpad,
  push: bool, codecs, max_size }`. `push` = the opener is the sharer
  (Share screen dialog, "With Desktop PC"); otherwise the opener wants to
  see or drive the target. `codecs` and `max_size` are the viewer's
  decode capabilities, so a push `Request` sends them empty.
- Target → opener: `Answer::{ Accept { session, codec, codecs, max_size },
  Decline, Expired, Busy, Unsupported, NeedsSetup }` (`NeedsSetup`:
  desktop permission, accessibility service or portal missing; mockup
  `kvm:portal`, `control:a11y`). Not push: the target is the sharer and
  sets `codec` (its pick from the `Request`'s `codecs`); `codecs` and
  `max_size` stay empty. Push: the target is the viewer, leaves `codec`
  unset and sends its own decode `codecs` and `max_size`; the sharer then
  picks the first it can encode (as below) before sending `Config`, or
  sends `Stop { Unsupported }` if none matches.
- Either way while live: `Paused { Locked | Protected }`, `Resumed`,
  `ControlAsk`, `ControlGranted`, `ControlTaken` (target took back
  control; mockup "Take back control", `viewer:revoked`), `Stop { reason
  }`, `Keyframe` (viewer asks, ≤ 1/s), `Feedback { rtt_ms, decode_ms,
  dropped }` (viewer, every 1 s), `Leave { pos }` (keyboard-and-mouse,
  target → source), `Heartbeat` (each side, whenever it has sent nothing
  else on this lane for 500 ms).

### `Input` events

`PointerAbs { x, y }` (0..=65535 normalised to the shared surface),
`PointerRel { dx, dy }`, `Button { button, down }`, `Scroll { dx, dy }`
(1/120 notch units), `Key { usage, down }` (USB HID usage code: the
physical key, the target's layout maps it), `Text { s }` (≤ 256 bytes;
phone keyboards produce text, not key codes), `Gesture { points }` (phone
target: a path of ≤ 64 `(x, y, t_ms)`), `System { Back | Home | Recents }`
(phone target), `ReleaseAll`. The target answers each batch with
`InputAck { seq }` on the same lane once it has injected or dropped it.
Coordinates outside the range are clamped;
anything else malformed ends the session.

## Policy (pinned)

| Feature | States | Meaning | Default: new phone link | Default: new computer link | Existing links |
|---|---|---|---|---|---|
| `screen` | `ask` / `on` / `off` | may this peer see **this** device's screen; `on` = "Always allow viewing" | `ask` (both sides) | `ask` | `off` |
| `input` | `ask` / `off` | may this peer drive **this** device (viewer control, touchpad, keyboard and mouse) | `off` (both sides: phone "Control this phone: Never", laptop "Keyboard and mouse: Never") | `off` | `off` |

- New group `Group::View` admits `ask`/`on`/`off` for `screen`; `input`
  joins `Group::Ask`. `Feature::PHASE3` = `PHASE2` + both. Older peers
  read both as reserved `Off` and show nothing.
- **Directional, not two-sided (Q8).** Phase 1's `allowed()` (both sides
  on, ask beats on) does not fit: the person on the device being seen or
  driven decides, and the person starting it already said yes by starting.
  So only the **target's own** `screen`/`input` gates and decides ask vs
  auto; the initiator's value is shown read-only ("Never on Pixel 8") and
  greys the action out. Push shares ask the viewer every time ("Desktop PC
  asks its user before it shows"); which viewer setting admits a push is
  open question 2.
- *Builder call (main thread), user may revisit:* for `screen` and
  `input` only the **target's** value counts; the initiator's own setting
  never blocks starting a session. The UI greys the action using the
  target's value ("Never on Pixel 8"). So `Supervisor::live()`
  (`crates/carrova-core/src/ecosystem/supervisor.rs:191-207`, today
  `allowed(local, remote)` for every feature) changes for these two
  features: it checks the remote (target) value, not the two-sided
  result.
- **Control always asks.** `input` has no `on`. "Always allow viewing"
  covers viewing only.
- Default for new links follows desktop Settings › Screen "Newly linked
  devices" (default Ask every time) and Android Settings › Screen and input
  (Ask to see · never control). `input` starts `off` everywhere (parent:
  "Keyboard-and-mouse control defaults Off").
- Global local settings (not policy, not sent): "Who can ask to see this
  screen: Linked devices / No one" (No one refuses every view request
  without asking), "Allow control requests" (off refuses every control and
  touchpad request without asking), "Share keyboard and mouse" (this
  computer acts as a keyboard-and-mouse source). Indicator and pause on
  lock are not settings (mockup: "Always on", "Always").
- Approvals: one per session, 60 s expiry, never remembered (Phase 1).
  Keyboard-and-mouse approval lasts "until you stop it or log out" on the
  target (mockup `kvm:request`) while the connection lasts; a reconnect
  needs a new yes (Q4).

## Sessions

### Lifecycle

```
idle → requested → (target asks | auto-allowed by screen=on) → OS prompt
     (portal / MediaProjection / none) → live ⇄ paused → ended
```

- **OS prompt after ours.** Wayland portal (`screen:portal`, `denied`) and
  Android MediaProjection (`share-screen:system`, `cancelled`) appear after
  Carrova's yes. Android 14+ asks MediaProjection consent every session, so
  "Always allow viewing" on a phone still shows Android's own prompt. On
  GNOME, "Remember this selection" stores a restore token
  (`<state>/screen/portal-token`, `0600`) so the desktop does not ask again
  until logout; forgetting the last device with `screen` on deletes it.
- **Live.** The target shows its indicator; the viewer shows the picture.
  Control starts only after `ControlAsk` → yes on the target (`screen:
  control`, `control:ask`). The target can take control back
  (`ControlTaken`) and keep the view.
- **Paused** while the target is locked (`Platform::screen_lock`, polled
  every 1 s during a session; `Unknown` counts as locked, failing closed as
  Phase 2 does). Sharer sends `Paused { Locked }`, stops frames, drops
  input and releases held keys; resumes on unlock without a new ask.
  Input does not wait for the poll: before injecting each batch the
  target reads the lock state (cached from `screen_lock`, refreshed on
  the platform's lock-change signal where there is one, else read per
  batch) and drops the batch and releases held keys unless it is
  `Unlocked`; `Unknown` drops it.
- **Ends** on Stop (either side, tray, notification, `carrova screen
  stop`), link loss (the next connection needs a new yes, Q4),
  forget, policy change to Off/Never, "Who can ask: No one", logout, app
  quit, MediaProjection stopped by Android, portal session closed by the
  desktop, accessibility service turned off (`control:a11yoff`). The end
  note says who watched and for how long, and for how long they had control
  (`screen:ended`). The activity list keeps kind, peer and duration only.

### Indicator (always on)

- Desktop: the share bar window (`sharebar`: "<Peer> is viewing |
  controlling this screen", timer, Allow control / Take back control,
  Stop) plus the tray's live item on top (`tray:sharing|controlled`).
- **GNOME Wayland cannot keep a client window always on top.** GNOME's own
  screen-sharing indicator (top bar, with its stop control) is shown for
  every portal session and is the guaranteed one there; Carrova's bar is
  shown as well but may be covered. Windows (later): WGC capture border
  stays on. macOS (later): the system recording indicator.
- Android: the system projection chip, Carrova's red status chip, Home's
  Stop card and an ongoing notification with Stop (`sys:sharing|
  controlled`); none can be turned off.
- No session runs without an indicator surface: the desktop app process
  must be running; `link serve` refuses (`Unsupported`).
- *Builder call (main thread), user may revisit:* the refusal is an
  explicit flag on `SupervisorConfig`
  (`crates/carrova-core/src/ecosystem/supervisor.rs:43-55`), e.g.
  `screen_sessions: bool`, true for the desktop app and false when
  `carrova link serve` builds the supervisor. With it false the gate
  refuses `Screen`, `Video` and `Input` lanes (`Unsupported` answer) and
  the supervisor offers no session commands. Test: a headless supervisor
  refuses a view request and a control request.

### Protected content (Q11)

- Windows (`SetWindowDisplayAffinity`), macOS (`NSWindow.sharingType`) and
  Android (`FLAG_SECURE`) exclude protected windows from capture; the
  viewer sees black. Carrova does not detect which window is protected, so
  `screen:protected` / `share-screen:secure` become a one-line note when a
  share starts: "Apps that protect their content show black to <Peer>."
- Linux has no protected-content mechanism under Wayland or X11; the note
  there says "Everything on this screen is visible to <Peer>."

## Keyboard and mouse (3d)

- **Arrangement** lives on the source only: `<state>/kvm/layout.json`,
  `0600`, `{ "version": 1, "screens": { "<peer hex>": { "x", "y", "w",
  "h" } } }` in logical pixels relative to this computer at the origin.
  Edges that touch lead to that peer (`kvm:arrange`, `dragging`). Each
  computer's desktop counts as one rectangle in v1 (multi-monitor = the
  bounding box).
- **Crossing.** The source sets barriers on arranged edges (InputCapture
  portal on Wayland; XInput2 pointer barriers and a grab on X11). On a hit
  it opens a `Kvm` session (asked once on the target, `kvm:request`), then
  sends `PointerAbs` at the mirrored entry point and relative motion,
  buttons, scroll and keys. The target owns its pointer position and sends
  `Leave { pos }` when the pointer goes back out of the entry edge; the
  source releases capture and warps its pointer to the matching point.
- **Hotkeys.** Scroll Lock comes back at once; Shift + Scroll Lock pins the
  pointer to this screen (`kvm:pinned`); Ctrl + Alt + F leaves the
  viewer's full screen (`viewer:full`). While input is captured the source
  sees every key, so the hotkeys work there; while the pointer is local,
  Wayland needs the GlobalShortcuts portal (not on GNOME 46): the tray's
  "Use Desktop PC" and the pin item are the fallback (Q12).
- **Never cross** while: pinned; a password field has focus on this
  computer (`kvm:secure`; macOS `IsSecureEventInputEnabled` later, Linux
  AT-SPI focused role `password text` best effort, Windows UI Automation
  `IsPassword` later); this computer is locked; the target is locked
  (always, per ADR 0005 §4; the mockup toggle "Only when the other
  computer is unlocked", `gui-ecosystem-desktop.html:1727`, is open
  question 3, recommended dropped); the target refused or is gone.
- **Peer gone**: link lost, lane dropped, a sent `Input` batch without
  its `InputAck` for 2 s (only unanswered batches count, so an idle
  source waits on nothing), or no message at all on the `Screen` lane for
  2 s (the 500 ms `Heartbeat` keeps an idle session up). Then the source
  releases capture and shows `kvm:gone`; the target releases all keys and
  buttons.
- **Clipboard along** (`kvm:clipboard`): reuses Phase 1 clipboard sync
  unchanged (the desktop already sends on change while `clipboard` is on
  for both). On a crossing Carrova only shows the toast if the last copy
  went across; no new mechanism (Q13).
- **Stuck keys.** On every exit (leave, stop, pause, lane drop, timeout)
  the target sends key-up for every key and button it pressed. Keys held on
  the source at crossing time are not sent.

## Phone specifics

- **Phone → computer mirror** (`share-screen`): the laptop requests, the
  phone asks, Android's MediaProjection prompt, then MediaCodec encodes
  (H.264 if the viewer decodes it, else VP8). `LinkService` adds the
  `mediaProjection` foreground-service type for the session's length.
  Android 15 already hides notification content and OTPs during
  projection.
- **Laptop drives the phone** (`control`): Carrova's `AccessibilityService`
  (declared with `canPerformGestures` and, on Android 13+,
  `flagInputMethodEditor`) does gestures (`dispatchGesture`), system keys
  (`performGlobalAction`) and text through `AccessibilityService.
  InputMethod` (`commitText`, `sendKeyEvent`). The service is enabled once
  by the user in Android settings (`control:a11y`; sideloaded apps first
  need "Allow restricted settings", the Phase 2 step), and does nothing
  unless a control session is live: it subscribes to no events
  (`eventTypes` none, no window content retrieval).
- **Computer → phone viewer** and **touchpad**: phone UI only; the
  computer captures/injects as in 3a/3b. Touchpad sends relative motion,
  two-finger scroll, tap = click; the modifier row sends `Key`, the soft
  keyboard sends `Text`.

## Media and performance budgets

Reference: Ubuntu 24.04 laptop (4+ cores, integrated GPU), 5 GHz Wi-Fi or
Ethernet, Android 13+ mid-range phone.

| Measure | Budget |
|---|---|
| Glass-to-glass, view, 1080p30 | p50 ≤ 80 ms, p95 ≤ 150 ms |
| Input to photon, control | p50 ≤ 120 ms |
| Keyboard-and-mouse event, source → injected | p50 ≤ 5 ms, p99 ≤ 20 ms; crossing ≤ 50 ms |
| Encode (VP8 software, 1080p) | ≤ 15 ms/frame, ≤ 1.5 cores |
| Bitrate | start 4 Mbit/s; adapt 0.5–12 Mbit/s |
| Static screen | frames only on damage, plus 1 fps keep-alive |
| Size | long edge capped at 1920 px in v1 (downscaled by the encoder path) |
| Queues | sharer ≤ 1 unacked frame (viewer acks each `seq` on the `Screen` lane, Q5); viewer ≤ 2 frames before decode |
| Idle | zero: no capture, encoder, hook or portal session without a live session |
| Phone mirroring battery | measured in 3a, recorded here; target ≤ 8 % per 30 min |

**Adaptive quality** (in core, OS-neutral): every second, from the
sharer's send backlog and the viewer's `Feedback`: backlog above one frame
or RTT doubling → halve bitrate and skip frames; 5 s clear → +10 %. Below
1 Mbit/s the encoder scales to 0.75× resolution, then 15 fps. Quality
drops before frames; the viewer shows "Lower quality · slow network" when
under the start bitrate (`viewer:slow`). A lost frame or decoder error →
`Keyframe`.

## Placement (respects the boundary rules)

- `carrova-link`: `screen.rs` (`Screen` lane messages), `video.rs`
  (frame header + raw bytes, sender drop policy), `input.rs` (events,
  limits, clamp); `LaneKind`, `Feature::PHASE3`, `Group::View`.
- `carrova-transport`: `Stream::set_priority` only.
- `carrova-core` `ecosystem/screen/`: sessions, asks, directional gate,
  lock pause, rate control, events for the indicator, end summary;
  `ecosystem/kvm/`: arrangement, crossing state machine, hotkeys, secure
  block, stuck-key release. Gate (`features.rs:116-137`) gains the three
  lanes.
- `carrova-platform` (only OS code), new trait methods with `Unsupported`
  defaults:
  - `screen_sources()`; `start_screen_capture(req) -> ScreenCapture`
    (guard yielding encoded frames; `set_target(bitrate, fps, scale)`,
    `keyframe()`; drop stops capture and closes the portal/projection);
  - `start_input_injection(target) -> InputSink` (guard; drop releases all
    keys);
  - `start_input_capture(barriers) -> InputCapture` (keyboard-and-mouse
    source; yields events and barrier hits);
  - `secure_input_active() -> Tristate`, `screen_lock()` (exists on Linux
    only, `linux/mod.rs:205`; the default is `Unknown`, `platform.rs:414`,
    which counts as locked).
  - *Builder call (main thread), user may revisit:* **Android lock query.**
    `AndroidPlatform::screen_lock` through the existing bridge
    (`KeyguardManager.isKeyguardLocked`, a Rust → Kotlin static call; no
    new export, no new `unsafe`). Until it exists every phone session
    would stay paused, so it lands in 3a before phone sharing.
  - Shared, OS-neutral `media/vp8.rs` (libvpx) behind a Cargo feature
    `screen`, enabled for **desktop builds only**. *Builder call (main
    thread), user may revisit:* Android builds do not link libvpx: phones
    encode with MediaCodec (VP8 or H.264) and decode in the WebView with
    WebCodecs. The CLI does not link it either.
  - Linux: `linux/capture/` (portal + PipeWire via `ashpd`/`pipewire`; X11
    `x11rb` SHM), `linux/inject/` (RemoteDesktop portal + `reis`; X11
    XTest via `x11rb`), `linux/kvm/` (InputCapture portal + `reis`; X11
    XInput2). Android: `android/capture.rs`, `android/control.rs` over the
    existing bridge. Fake: synthetic frames (a counter pattern, passthrough
    "codec"), recording injector, scripted barrier hits, lock and
    secure-input switches.
  - Windows/macOS later (named for the plan): Windows.Graphics.Capture or
    DXGI Desktop Duplication, Media Foundation H.264, `SendInput` (UIPI:
    no elevated windows, no secure desktop), low-level hooks for capture;
    macOS ScreenCaptureKit, VideoToolbox, `CGEventPost` (Accessibility
    permission), `CGEventTap` (Input Monitoring).
- `carrova-view`: IPC contract (session events, commands: request, answer,
  allow control, take back, stop, arrangement get/set; a raw-bytes frame
  channel).
- `src-tauri`: forwards encoded frames to the viewer window through a
  Tauri `ipc::Channel` (raw bytes), opens the share bar window; no logic.
- Front-end: viewer (WebCodecs → canvas, pointer lock in control, keyboard
  capture minus Ctrl + Alt + F), share dialog, share bar, arrange view,
  settings rows; phone viewer, touchpad, control and share screens.
- Kotlin glue (no decisions): `Capture.kt` (MediaProjection consent
  activity, MediaCodec → pipe fd), `ControlService.kt`
  (`AccessibilityService`), `Control.kt` (gesture/text/system calls),
  manifest entries; `Perms.kt` gains accessibility status.

## CLI (pinned)

`carrova devices features <device> screen=ask|on|off input=ask|off`,
`carrova link status` shows live sessions, and `carrova screen stop`
(ends every screen, control and keyboard-and-mouse session; a terminal
escape hatch). Nothing else: viewing is a UI job.

## Security and privacy (pinned)

- Only linked devices, on their authenticated link connection, can request
  (ADR 0002); forgotten devices cannot connect at all.
- Consent per session on the target; control is a second yes; no "always"
  for control; asks expire in 60 s and are never remembered.
- Indicator always on while anything is seen or driven; pause on lock;
  protected content handled as above.
- Input is injected only while an approved session is live, only from the
  connection it was approved on, only when the target is unlocked; rate
  and size limits as pinned; malformed input ends the session; all keys
  released on every exit.
- Typed secrets do not cross: secure-input and lock blocks above.
- Never logged or reported: frames, frame hashes, typed text, key codes,
  pointer positions, window titles, the portal restore token. `tracing`
  fields: codec, size, bitrate, fps, counts, durations, end reason. A
  redaction test runs a failing session and asserts the report holds no
  frame byte, key or text.
- Nothing is recorded or stored: frames live in memory, ≤ the queues above.

## Testing

- **Unit** (`tests/<module>/unit.rs`): lane round-trips, caps, unknown
  versions; input clamp and rate limit; rate controller against a
  simulated backlog; crossing state machine (edges, pin, secure, lock,
  gone, stuck keys); arrangement geometry; directional policy.
- **Two supervisors in one process** with the fake platform: ask → allow /
  decline / expire; "Always allow viewing" skips the ask but still emits
  the indicator event; control ask and take back; lock pauses frames and
  drops input; `lock_during_input_injects_nothing` (lock between two
  batches: the next batch is not injected); link drop ends control;
  `idle_session_survives_on_heartbeat`, `lost_peer_detected_without_input`;
  `push_share_uses_viewer_codecs`; forget, Off, "No one" end sessions;
  `Video`/`Input` lanes without a live session refused; input after Stop
  dropped; full keyboard-and-mouse round trip; push share asks the viewer;
  the initiator's own `screen`/`input` = Never does not block it starting
  a session; a headless supervisor (`SupervisorConfig` flag off) refuses
  view and control.
- **Android lock query**: `screen_lock` maps the bridge's keyguard answer
  (locked, unlocked, error → `Unknown`); a phone share pauses on lock and
  resumes on unlock (fake bridge).
- **Front-end**: Vitest for viewer, share bar, arrange with a fake frame
  channel and mocked `VideoDecoder`; Playwright over the IPC fake for every
  mocked state.
- **Containers**: `just e2e --screen`: two containers, Xvfb as X11 source
  and target, real libvpx; one views and controls the other; checks a test
  pattern arrives and an injected click lands. Wayland portals and Android
  are covered on real hardware only.
- **Real hardware** (user; exit gate below).

## Acceptance (Phase 3 exit gate)

Automated: `just ci` green, `just e2e --link` and `just e2e --screen` PASS,
output shown; the tests above.

Real hardware (user, Ubuntu 24.04 GNOME Wayland laptop + second Ubuntu
computer + Android 13+ phone):

- View and control the second computer; text readable; take back control;
  lock it → paused; Stop from the bar and from GNOME's indicator.
- Mirror the phone on the laptop; control it (accessibility set up once);
  open a banking app → black; lock the phone → paused.
- View and control the laptop from the phone.
- Phone as touchpad and keyboard.
- Arrange two computers; cross both ways; type on both; pin; a password
  field keeps the pointer home; pull the other's network → pointer home,
  no stuck keys.
- Budgets: glass-to-glass p50 and keyboard-and-mouse p50 measured and
  recorded; phone battery for 30 min mirroring recorded.

## Risks

| Risk | Mitigation |
|---|---|
| GNOME Wayland will not keep Carrova's bar on top | GNOME's own sharing indicator is shown for every portal session and has Stop; Carrova's bar and tray add to it; documented in the UI note |
| InputCapture portal is young; GNOME asks at every start; no GlobalShortcuts on GNOME 46 | 3d last, spike S6 first; tray fallback for hotkeys; drop crossing from the gate if S6 fails |
| Webview cannot decode a codec (WebKitGTK + GStreamer plugins) | Viewer advertises only what decodes; VP8 baseline uses default Ubuntu plugins; S2 decides; Rust-decode fallback planned only if needed |
| Software VP8 too heavy on HiDPI/4K laptops | Cap at 1920 px, damage-only frames, adaptive fps; hardware H.264 on later adapters |
| Android: MediaProjection prompt every session; accessibility needs restricted-settings step; Play policy | Shown in the flow (mockups have the states); APK distribution as today; service inert outside sessions |
| Lost packets stall the stream on bad Wi-Fi (head-of-line) | Drop-to-latest on the sender, keyframe on request; datagrams are the documented next step |
| Secure-input detection is best effort on Linux | Lock screen always blocks; pin hotkey; the risk is stated in the kvm settings note |
| Stuck keys after failures | Target-side release on every exit and when the peer is gone (2 s without `Screen` lane messages; the heartbeat keeps a held key held) |
| Laptops without Scroll Lock | Crossing back by edge always works; tray items (Q12) |
| H.264 patents | Never ship an encoder; OS/device encoders only |
| Lock state `Unknown` on some desktops | Counts as locked: no sharing there (fail closed), said plainly in the share dialog |

## Open questions for the user (each with my recommendation)

Questions 1–5 came from the builder check (2026-11-20) and block ADR
0005's sign-off; answer them first.

1. **Lane session id and push gating.** `LaneGate::admit` sees only the
   kind, before anything else is read (`crates/carrova-link/src/
   lane.rs:76-79`, `133-157`), so it cannot check a `Video`/`Input`
   lane's session id. Options: (a) an optional `session: Option<[u8; 16]>`
   field on `LaneOpen` (`#[serde(default)]`; older peers ignore it, as
   `WireOpen` already tolerates unknown fields) and `admit(kind,
   session)`; (b) the gate reads one more frame after `LaneOpen`; (c)
   accept, then read the id and close on mismatch. *Recommend (a):
   checked before any other byte is read, one frame, no extra round trip,
   no change for existing lanes. The `Screen` lane carries no id; its
   gate checks only that some screen/input setting for the peer is not
   off, then the `Request` is checked after reading.*
2. **Which viewer-side setting admits a push share.** The viewer's
   `screen` means "may they see me", so it does not fit; without a rule,
   any linked device could make another ask even with every feature off,
   contradicting the parent design ("connect only once a feature is
   on"). *Recommend: make the per-device `screen` row mean "Screen
   sharing with this device" (the desktop row's title): `off` = nothing
   screen-related either way, so a push is refused unasked; `ask`/`on`
   admit a push, which still always asks the viewer; `ask` vs `on` only
   decide viewing of this device. One key, no new setting.*
3. **Keyboard and mouse into a locked target.** The mockup's "Only when
   the other computer is unlocked" toggle
   (`gui-ecosystem-desktop.html:1727`) contradicts ADR 0005 §4 (input
   only while unlocked). *Recommend: drop the toggle; always
   unlocked-only (you cannot type the other computer's password through
   Carrova; unlock it at its own keyboard).*
4. **Reconnect.** Old Q9/Q10 let approvals survive a reconnect, but ADR
   0005 §1/§3 say a session never outlives its connection, and the
   anti-goals require a fresh yes for every session that drives a device
   (and for every view unless "Always allow viewing" is on). *Recommend: a
   session always ends with its connection and nothing is carried over:
   every keyboard-and-mouse, control and touchpad session after a
   reconnect needs a fresh yes on the target. View-only resumes without
   an ask only where "Always allow viewing" already skips it (indicator
   shows again); a 30 s view-only grace without that setting would
   conflict with the first anti-goal, so it is not recommended unless you
   amend that anti-goal.*
5. **Transport scope of sign-off.** `Stream` is one `&mut` for send and
   receive with no split (`crates/carrova-transport/src/stream.rs:23-88`),
   and the 4 MiB per-stream window (`tuning.rs:28`) holds about 8 s of
   video, so "≤ 1 frame waiting" cannot be enforced by write blocking or
   priority alone. Options: (a) `Stream::split` plus a backlog signal
   and/or a smaller `Video` window, covered by ADR 0005; (b) frame acks on
   the `Screen` lane. *Recommend (b): the viewer acks each decoded frame
   `seq` on the `Screen` lane; the sharer keeps at most one unacked frame
   and skips the rest. It measures the real end to end (network + decode),
   feeds the rate controller, and needs no transport change beyond
   `set_priority`. The sharer's session loop polls `recv` with a short
   timeout between sends (a timeout loses nothing, `stream.rs:38-49`).*

6. **Media transport.** One QUIC stream per session (drop-to-latest), no
   datagrams in v1. *Recommend: yes; datagrams only if 3a hardware shows
   stalls (needs a new ADR).*
7. **Codec.** VP8 software baseline + H.264 only from OS/device encoders;
   never ship openh264. *Recommend: yes.*
8. **Directional policy** for `screen`/`input` (only the target's setting
   gates; initiator's shown read-only), unlike Phase 1's two-sided rule.
   *Recommend: yes; it is what the mockups' "Asks happen on the device
   being shared, never on the one asking" means.*
9. **Keyboard-and-mouse approval scope** (superseded by Q4): until Stop or logout on the
   target, surviving short link drops (a reconnect needs the ADR 0002
   proofs from the same key). Viewer control and touchpad end on any link
   loss. *Recommend: yes, as the mockup's "Until you stop it or log out".*
10. **Reconnect grace for view-only** (superseded by Q4): resume within 30 s without a new ask
   (`viewer:lost` "Trying again…"); control never resumes. *Recommend: yes.*
11. **Protected content**: no live detection; a one-line note at share
   start; on Linux "everything is visible". Mockup states `screen:
   protected` and `share-screen:secure` become that note. *Recommend: yes.*
12. **Hotkeys**: keep Scroll Lock / Shift + Scroll Lock / Ctrl + Alt + F as
   mocked, with tray fallbacks; rebinding later. *Recommend: yes; revisit
   when GNOME ships GlobalShortcuts.*
13. **"Clipboard follows the pointer" toggle**: redundant with Phase 1
   clipboard sync. *Recommend: drop the toggle, keep the toast (avoids two
   settings for one thing).*
14. **"Allow control requests" global toggle** beside per-device `input`.
   *Recommend: keep as a global kill switch (default on); per device stays
   the main control.*
15. **Headless `link serve`** refuses screen and input. *Recommend: yes
    (no indicator surface).*
16. **Lock state `Unknown` = no sharing** (fail closed). *Recommend: yes;
    GNOME and KDE report it.*
17. **Sub-phase order** 3a → 3b → 3c → 3d, with 3d droppable to its own
    plan if S6 fails. *Recommend: yes.*
18. **Audio** out of v1. *Recommend: yes; separate small plan after 3a.*
19. **Default `screen` for new computer links = ask** (Phase 1 made new
    computer links all off). *Recommend: ask, per Settings "Newly linked
    devices"; existing links stay off.*

## Left to the builder

- CBOR field layout inside the pinned messages; exact AIMD constants within
  the budgets.
- Crate versions (`ashpd`, `pipewire`, `reis`, `vpx` bindings), checked for
  maintenance and licence (MIT/Apache/BSD) in the plan.
- HID-usage ↔ evdev/keysym/Android key tables.
- Share bar window placement; arrange-view snapping.
- Order of Android permission prompts inside the mocked states.
- Activity-list wording for ended sessions.

## Spike results

- None yet.
