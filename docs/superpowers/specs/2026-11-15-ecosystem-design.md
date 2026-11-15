# Carrova ecosystem: devices that work together

Date: 2026-11-15. Status: approved by the user 2026-11-15. Phase 1 detail: `2026-11-15-ecosystem-phase1-design.md`.
Mockups (source of truth for every screen):
`docs/mockups/gui-ecosystem-desktop.html` (23 screens, 87 states) and
`docs/mockups/gui-ecosystem-android.html` (20 screens, 75 states), both
built on the approved `docs/mockups/gui-v4.html` framework. UI approved by
the user 2026-11-15.

## Why

The user wants Carrova to give non-Apple devices what Apple's ecosystem
gives Apple users: answer calls and messages on the laptop, copy on one
device and paste on another, share screens, share a keyboard and mouse,
hand work from one device to the next. No single tool does this well across
Linux, Windows, macOS and Android today (KDE Connect covers part of it with
rough UX and no screen share; Phone Link is Windows-only and closed).

Carrova already has the hard foundation: per-install device identity keys,
code pairing with PAKE, known devices, mDNS presence, an encrypted QUIC
transport and a background listener. The ecosystem builds on it. Migration
stays, as one feature of the ecosystem ("set up a new computer").

## Decisions made by the user

| Topic | Decision |
|---|---|
| Direction | Apple-like ecosystem; migration kept as a feature |
| Phones | Android first. iOS deferred (Apple blocks messages, calls, background clipboard for third-party apps) |
| Laptops | Linux first, then Windows, then macOS, through the existing platform boundary |
| Network | Same network only. The link runs over the LAN; Bluetooth is used only for call audio (HFP), never as a link transport. No relay, accounts or cloud |
| Approach | Own protocol + the existing Tauri 2 app, built for Android too (approach 1 below) |
| Trust split | **Quiet once switched on:** clipboard, notifications, messages, calls ringing. **Ask every time:** files, seeing a screen, controlling keyboard/mouse, Handoff. Per device |
| Calls | Two steps (user, 2026-11-15). **Base, no Bluetooth:** ring on the laptop, answer/decline/end/mute and call back from the laptop over the link; you talk on the phone (speaker on answer). **Optional audio:** "Use this laptop's mic and speakers" makes the laptop a Bluetooth headset (HFP); on Linux the wizard detects and asks for a one-time package install (oFono), Ubuntu 24.04 LTS and newer. Wi-Fi call audio is impossible: Android lets only system apps capture call audio |
| Order | Foundation → clipboard + files → messages → calls → screen → keyboard and mouse → Handoff, grouped into the 3 phases below |
| UI | Both ecosystem mockups as built |
| Plan size | At most 3 phases |

## Goals

1. Pair an Android phone with a computer by scanning a QR code (typed code
   as fallback). Computers pair with each other as today.
2. Linked devices on the same network connect by themselves, stay connected
   while near, and reconnect silently. One encrypted connection per pair,
   carrying separate lanes per feature.
3. Per device, per feature switches: quiet features On/Off, ask features
   Ask/Off. Enforced by the device being acted on, never by the asker.
4. Features, as in the mockups: shared clipboard (with history and password
   skip), file send, Handoff (links and files), phone notifications with
   inline reply, messages (SMS), calls on the laptop, Ring (find my phone),
   screen sharing and viewing (phone and computer, view and control), shared
   keyboard and mouse between computers, phone as touchpad and keyboard.
5. Migration unchanged and reachable from the device page of a computer.
6. CLI and app behave the same where both apply (`carrova devices`, link
   status, feature switches, send file, send clipboard).

## Non-goals

- iOS, iPadOS, Apple Watch, tablets, landscape phone layouts.
- Any relay, account, cloud sync, or use away from the local network.
- iMessage, RCS chats, reading chat apps' message stores (chat apps work
  only through notification reply).
- Copying apps or app data to or from the phone (migration stays
  computer-to-computer).
- Play Store listing in this plan. Distribution is a signed APK (GitHub
  release); Play Store review of SMS/Accessibility permissions is a later
  decision.
- Flathub and macOS notarisation (still deferred).

## Anti-goals (failure even if it works)

- A device that is not linked, or a stranger on the Wi-Fi, can make anything
  appear, ring, open, or connect.
- A screen is seen or a computer driven without a visible, stoppable
  indicator and a fresh approval on that device.
- Forgetting a device leaves it able to reconnect.
- Message text, notification text, clipboard contents, call numbers or
  screen pixels end up in logs, crash reports or error reports.
- The phone's battery noticeably drops because of Carrova while idle, or
  the laptop's idle CPU rises.
- Anything said twice on one screen; settings that disagree (the quiet
  instrument rules of `2026-11-12-background-nearby-design.md` hold).

## Constraints

- Project rules (CLAUDE.md) hold: OS-specific code only in
  `carrova-platform`; `src-tauri` has no business logic; dependencies point
  down (front-ends → core → services → platform); thiserror in libraries;
  tests outside `src/`; never log contents, keys or codes; `just ci` green.
- **Any change to `carrova-crypto` or the transport handshake needs the
  user's explicit sign-off, through a written ADR, before code.** Phase 1
  starts with ADR 0002 (linked connections). Calls (Bluetooth) and screen
  streams get ADRs if they touch the handshake or crypto.
- Audited crates only for crypto and codecs; no hand-rolled primitives.
- Android: Kotlin is allowed only for Android API glue reached through the
  `Platform` trait (see "Android shell"). Business logic stays in Rust.
- Linux Wayland: screen capture and input injection go through
  xdg-desktop-portal (ScreenCast, RemoteDesktop); the OS prompt is part of
  the flow (mockups show it).

## Approaches considered

| Approach | Trade-off | Verdict |
|---|---|---|
| **1. Own protocol + Tauri 2 Android build** of the existing app; Android-only parts as small Kotlin glue behind the platform trait | One codebase, one IPC contract, reuses UI, view-models, i18n and the whole Rust engine. Tauri mobile is younger; some background work needs Kotlin | **Picked** |
| 2. Own protocol + native Kotlin/Compose app, Rust core via UniFFI | Best Android feel, but a second UI to keep in sync and breaks "CLI and UI behave identically through one contract" | Rejected |
| 3. Speak the KDE Connect protocol, use its Android app | Fastest start, but weaker trust model (accept on first sight, no PAKE), no screen share, no Carrova on the phone, migration cannot extend | Rejected |

## Design

### How it fits the crates

```
apps/desktop (React UI: desktop + phone layouts)     apps/cli
apps/desktop/src-tauri (bridge only; Android target)  │
        └──────────────► carrova-core ◄───────────────┘
                          ecosystem/ module: link supervisor, policy, events
                               │
              ┌────────────────┼──────────────────────┐
        carrova-link (new)  carrova-transport      carrova-view
        lanes + feature     + Link handshake       ecosystem view-models,
        protocols           (ADR 0002)             IPC contract (ts-rs)
              │                 │
              └──────► carrova-crypto (KnownPurpose::Link, link proof)
                          │
                    carrova-platform  (+ android adapter; clipboard,
                     notifications, SMS, Bluetooth, screen, input traits)
```

- **`carrova-link` (new service crate):** lane framing and one protocol per
  feature (clipboard, file offer, handoff, notify, messages, calls control,
  screen, input, ring, ping). Depends on transport, crypto, platform; never
  on core.
- **`carrova-core::ecosystem`:** owns the link supervisor (who to connect,
  when, backoff), the per-device policy, ask approvals, and emits typed
  events. Front-ends render events and send intents; they never poke state.
- **`carrova-view`:** ecosystem view-models and the IPC contract, generated
  to TypeScript like today.
- **`carrova-platform`:** new trait methods per OS capability (clipboard
  read/write and change watch, show/act on notifications, read/send SMS,
  Bluetooth HFP, screen capture, input injection, multicast lock, keep
  alive). Linux first; Windows/macOS methods return `Unsupported` until
  their turn; Android adapter added.

### 1. Pairing and linked connections (ADR 0002, needs sign-off)

**Pair-only (builder check: today pairing exists only inside a move).**
ADR 0002 also adds a pair-only path: `Hello { intent: Pair { nameplate },
identify: true }` followed by a new `PairOnly` purpose, so the session ends
right after `Identify` and the `KnownSecret` is stored. No move session,
no fake empty manifest. Computers use the same path from Nearby →
"Link a computer"; a move still remembers devices as today.

**QR payload (my call):** `carrova://pair?v=1&t=<ticket>&k=<laptop public
key, base32>` where `<ticket>` is the existing `<code>@<ipv4>:<port>`
format. The phone pins the key: pairing fails if the laptop's `Identify`
key differs. The typed-code fallback works without the pin, as today.


Today a known device can only be *recognised* (`KnownPurpose::Discovery`
tags); there is no code-less authenticated connection (the pending "Phase 3
code-less requests" of the background-nearby plan). The ecosystem needs it,
and it also unblocks code-less send.

Proposal for the ADR (the ADR decides the details):

- New `Hello` intent `Link { nonce, tag }` with
  `KnownPurpose::Link` (own HMAC label, never verifies as a discovery tag).
- Mutual authentication over the TLS channel binding, with both the
  `KnownSecret` (HMAC proof, as resume does) and each side's device key
  (Ed25519 signature under a new `SignContext::DeviceLink`, role-bound, as
  `Identify` does). Either proof failing closes the connection.
- Replay: nonces admitted once per listener (existing `ResumeNonces`
  pattern).
- Forgetting a device deletes its `KnownSecret` and drops any live link at
  once; the other side's next attempt fails.
- Who dials: the device with the lower public key dials, the other listens;
  if both connected, the dialer's connection wins. (Builder may change the
  tie-break, but there must be exactly one connection per pair.)

### 2. The link

- **One supervisor per user (my call):** the running Carrova (app
  background listener, or `carrova link serve` when only the CLI is used)
  owns all links; a per-user single-instance lock ensures one. CLI commands
  talk to it over a local control socket in the state dir (mode `0600`,
  owner-checked, same rules as other state files) and, if none runs, start
  a short-lived supervisor for that command. This keeps exactly one
  connection per pair. The control socket gets a security review in the
  Phase 1 plan.

- One QUIC connection per linked pair, opened when presence shows the peer
  and at least one feature is on, closed after a quiet period when no
  feature needs it.
- Lanes = QUIC bidirectional streams opened with a typed `LaneOpen
  { kind, version }` frame; unknown kind or version is refused, not fatal.
  Control messages CBOR with a length cap (existing framing); bulk data
  (files, screen frames) raw on their own streams.
- Keepalive and reconnect with backoff tuned per platform (Android longer
  keepalive to save battery). Network change (Wi-Fi switch, sleep/wake)
  triggers a fresh presence check and reconnect.
- Every incoming lane is checked against the local policy for that peer
  before any data is read past the open frame.

### 3. Policy and approvals

- **Two-sided (my call, matches the mockups, where both devices show the
  switches):** each device keeps its own policy for each peer. The device
  that *gives* data (the phone for notifications and SMS, the copying side
  for clipboard) decides what leaves it; the device that *receives* decides
  what it accepts and shows. A feature runs only when both sides have it on.
  Each side's switch state is sent to the peer on link open and on change
  (a `policy` lane), shown read-only there ("Off on Pixel 8"), so the two
  screens never disagree silently. Policy is never changed remotely.
- **Existing known computers (my call):** become linked devices with every
  feature Off. Because the link opens only when a feature is on, nothing
  connects until the user switches something on. `DeviceRecord` gains no
  separate "known vs linked" state; the policy file carries it.

- Policy is stored per device beside `DeviceRecord` (full public key as
  key), versioned, shared by CLI and app.
- Quiet features: On/Off. Ask features: Ask/Off. Keyboard-and-mouse control
  defaults Off; other ask features default Ask; quiet features are chosen
  on the phone's "Choose what to share" screen right after pairing (my
  call: clipboard and notifications pre-selected, messages and calls not,
  user can change all).
- An ask creates one approval scoped to one session (one file offer, one
  screen view, one control session, one handoff). Approvals expire if not
  answered (default 60 s) and never carry over.
- While a screen is viewed or controlled, a visible stop bar shows on the
  shared device (mockups); Android uses the system capture chip and a
  notification.

### 4. Android shell

- The existing Tauri app gains the Android target (`tauri android`), same
  React code with the phone layouts from the mockup, same IPC contract.
  Minimum Android 10 (API 29) (my call: clipboard and background rules are
  consistent from there).
- A Kotlin foreground service (type `connectedDevice`) hosts the Rust link
  supervisor so the link lives when the UI is closed, shows the "Linked"
  notification, holds a multicast lock for mDNS while active, and offers the
  quick-settings tile and share-sheet targets.
- Android API glue (clipboard, NotificationListenerService, SMS,
  Bluetooth, MediaProjection, AccessibilityService) is Kotlin behind
  `carrova-platform` trait methods, called through a narrow JNI bridge owned
  by `carrova-platform`. The exact bridge mechanism goes in the Phase 1
  plan.
- Permissions are requested only when the matching feature is turned on.
- Android 13+ marks notification access and Accessibility as "restricted
  settings" for sideloaded apps. Onboarding adds the "Allow restricted
  settings" step (App info → ⋮) before those two permissions; the Phase 1
  plan adds that state to the Android mockup.
- Distribution: signed APK on GitHub releases, with in-app update check
  like the desktop updater.

### 5. Features (behaviour as in the mockups)

- **Clipboard:** laptop→phone automatic; phone→laptop by tap (tile,
  notification action, share sheet) because Android 10+ blocks background
  reads. Text, links and images; history (last 20, local only, clearable);
  items flagged as passwords by the OS are not sent.
- **Files:** offer → ask → stream with resume over a file lane; saved to
  Downloads; Android share sheet target.
- **Handoff:** send a link or file "to continue" on another device; it asks
  there and opens with the default app. App-state handoff is out.
- **Notifications:** phone notifications mirrored to the laptop with inline
  reply where the app supports it; per-app list, banking-type apps off by
  default; nothing shown on a locked laptop screen beyond "Pixel 8: new
  notification" unless opted in.
- **Messages:** SMS read/send via Android SMS permission, conversation list
  and threads on the laptop; RCS chats are not visible (note shown).
- **Calls, base (no Bluetooth):** the phone reports call state over the
  link; the laptop shows the ringing pop-up, call window and history.
  Answer/end via `ANSWER_PHONE_CALLS`, answering turns on the phone's
  speaker; mute via the phone's audio manager; call back dials on the phone
  (`CALL_PHONE`). Needs only the phone permissions.
- **Calls, optional audio:** "Use this laptop's mic and speakers" runs the
  one-time Bluetooth wizard; the laptop registers as a hands-free device
  (Linux: BlueZ + PipeWire with oFono; the wizard checks for it and shows a
  one-step install when missing; Ubuntu 24.04 LTS and newer). Keypad and
  move-to-phone need this mode.
- Carrova is never the default dialer.
- **Ring:** make the phone ring at full volume from the laptop.
- **Screen:** view another device's screen; ask for control. Computers via
  portal/OS capture, phone via MediaProjection; control of a phone needs
  AccessibilityService. Hardware video encode where available, software
  fallback (codec crate chosen in Phase 3's plan, audited and maintained).
- **Keyboard and mouse:** arrange screens; pointer crosses edges between
  computers; phone as touchpad and keyboard for a computer. Linux input
  through the RemoteDesktop portal (libei) on Wayland, XTest on X11.

## Roadmap: three phases

Each phase ends with `just ci` green and a real-hardware exit gate
(Linux laptop + Android phone; second computer where needed).

### Phase 1 — Link, phone app, clipboard, files, Handoff

- ADR 0002 linked connections → user sign-off → handshake + crypto changes.
- `carrova-link`, `core::ecosystem`, policy store, view-models, CLI
  (`carrova devices`, `carrova link status`, `carrova devices features`,
  `carrova send-clip`, `carrova send-file --to`).
- Android build: shell, foreground service, QR pairing, onboarding,
  home, device page, activity, settings, system surfaces.
- Desktop: Nearby with phones, Add a phone, device pages, Home glance, tray,
  settings sections.
- Features: clipboard, files, Handoff (links + files), Ring.
- Exit gate: pair by QR; link survives Wi-Fi drop, laptop sleep, app
  swipe-away; clipboard both ways; file both ways; idle phone battery
  measured over 8 h.

### Phase 2 — Phone on the laptop: notifications, messages, calls

- Notifications mirror + inline reply, per-app list.
- Messages (SMS) with conversations and compose.
- Calls base: ringing pop-up, answer/decline/end/mute, call back, call
  history, over the link.
- Calls optional audio: Bluetooth wizard (with the oFono install step),
  HFP on Linux, keypad, move to phone, Bluetooth-off and unsupported states.
  The Phase 2 plan adds the "install one package" and "talk on the phone"
  states to the desktop mockup.
- Exit gate: reply to a chat notification and an SMS from the laptop;
  answer and end a call from the laptop with no Bluetooth; with audio set up
  on Ubuntu 24.04, hold a 5-minute call on the laptop with clear audio both
  ways.

### Phase 3 — Screens and input

- Screen sharing and viewing (computer↔computer, phone→computer,
  computer→phone viewer), control requests, stop bars, Wayland portal flow.
- Shared keyboard and mouse between computers, screen arrangement, phone as
  touchpad and keyboard.
- ADR if screen or input streams need crypto/handshake changes.
- Exit gate: view and control a second computer on GNOME Wayland; mirror the
  phone; move the pointer across two computers and type on both.

Windows and macOS adapters for link features follow each phase where the
platform allows, without blocking the Linux + Android exit gates.

## Documentation to update when Phase 1 starts

- `CLAUDE.md` status and v1 scope line (now: Ubuntu + Android ecosystem on
  the LAN, plus migration), layout (new `carrova-link`).
- `docs/PLAN.md`: new section "Ecosystem" with the three phases.
- The HLD stays as the migration design; this spec is the ecosystem design.

## Left to the builder

- Lane frame layout, CBOR schemas, keepalive numbers, backoff numbers.
- JNI bridge mechanism and Kotlin module layout (within the rules above).
- Choice of Bluetooth, codec and portal crates (audited, maintained), named
  in the phase plans.
- Exact Android permission request order inside the mocked screens.
- Test harness for two link instances in one process (as session tests do).
