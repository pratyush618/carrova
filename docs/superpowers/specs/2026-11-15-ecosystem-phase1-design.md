# Ecosystem Phase 1: link, phone app, clipboard, files, Handoff, Ring

Date: 2026-11-15. Status: approved by the user 2026-11-15 (with the parent
design). Parent design (decisions, approaches, roadmap):
`docs/superpowers/specs/2026-11-15-ecosystem-design.md`. Screens:
`docs/mockups/gui-ecosystem-desktop.html` and
`docs/mockups/gui-ecosystem-android.html` (the screens listed below).
Plan: `docs/superpowers/plans/2026-11-15-ecosystem-phase1.md`.

This document narrows the parent design to Phase 1 and pins what the
parent left open for it. Where they differ, this one wins for Phase 1.

## What Phase 1 delivers

A Linux laptop and an Android phone (and two Linux computers) that the user
links once, which then stay connected on the same network and let the user:

1. copy on one and paste on the other (clipboard),
2. send a file either way (ask on arrival),
3. send a link or file "to continue" on the other (Handoff, ask on arrival),
4. make the phone ring from the laptop (Ring),

with per-device switches on both sides, from the desktop app, the Android
app and the CLI.

## Out of Phase 1

Notifications, messages, calls (Phase 2); screen, keyboard and mouse,
touchpad (Phase 3); iOS; Windows/macOS link adapters beyond compiling
(their clipboard/notify methods return `Unsupported`); Play Store;
redesign of History and Apps.

## Gate 0: ADR 0002 before any handshake or crypto code

`docs/adr/0002-linked-devices.md` is written first and needs the user's
explicit sign-off (CLAUDE.md "Crypto"). It covers exactly:

1. **Pair-only.** `Hello { proto: 1, intent: Pair { nameplate }, identify:
   true, purpose: PairOnly }` (new optional `purpose`, default `Move` for
   older peers). After both `Identify` frames and `Established`, the sender
   sends `Linked` and both close. Both store the `KnownSecret` and the
   record. A pair-only `Hello` without `identify: true` is rejected.
2. **QR payload.** `carrova://pair?v=1&t=<ticket>&k=<base32 DevicePublic>`.
   `<ticket>` is the existing `Ticket` text. When `k` is present the
   scanning side fails the pairing if the peer's `Identify` key differs
   (error `KeyMismatch`, UI "This isn't the computer you scanned").
3. **Link intent.** `Intent::Link { nonce: [u8; 16], tag: [u8; 16] }`,
   `tag = HMAC(KnownSecret, "carrova/1/known-link" ‖ nonce ‖ dialer's
   DevicePublic)[..16]` (`KnownPurpose::Link`). The listener finds the
   secret by trying each known (secret, peer key) pair in constant time,
   admits each nonce once (`ResumeNonces` pattern), then both sides prove:
   `HMAC(KnownSecret, role ‖ ChannelBinding)` and an Ed25519 signature under
   `SignContext::DeviceLink` (`carrova/device-link/v1\0`) over `role ‖
   ChannelBinding ‖ own key`. Any failure closes with `LinkFailed`; no
   detail is sent to the peer.
4. **One connection per pair.** The lower `DevicePublic` (bytewise) dials.
   If a duplicate exists the newest connection from the dialer wins and the
   older one is closed with `Superseded`.
5. **Forget.** Deleting the record deletes the secret and drops the live
   link at once; a link attempt with no matching secret is rejected the
   same way as a bad tag.
6. **Threat model additions:** stolen phone on the LAN (can link until
   forgotten; ask features still ask on the laptop), replayed `Hello`
   (nonce cache), downgraded peer (a `Link` intent from `proto` < 1 is
   rejected), tag linkability (fresh nonce per attempt).

Nothing in tasks after the ADR may change these points without a new
sign-off.

## The link (pinned values)

| Item | Value |
|---|---|
| Dial when | peer seen by presence **and** at least one feature is on for it on this side |
| Close when | no feature on, or peer gone for 60 s, or forget |
| QUIC keepalive | desktop 15 s; Android 45 s |
| Idle timeout | 3× keepalive |
| Reconnect backoff | 1 s doubling to 60 s, reset on network change |
| Lanes | QUIC bidirectional streams; first frame `LaneOpen { kind, version: 1 }` |
| Lane kinds (Phase 1) | `Policy`, `Clipboard`, `File`, `Handoff`, `Ring`, `Ping` |
| Unknown kind/version | `LaneRefused { reason: Unknown }`, connection stays up |
| Control frame cap | existing `MAX_FRAME_LEN` |
| Lane check | local policy for this peer checked before reading past `LaneOpen` |

## Policy (pinned)

- File: `<state>/devices/policy/<hex public key>.json`, mode `0600`,
  written atomically, schema `{ "version": 1, "features": { "<feature>":
  "<state>" } }`.
- Features and states:

| Feature | Group | States | Default for a new phone link | Default for an existing known computer |
|---|---|---|---|---|
| `clipboard` | quiet | `on`/`off` | on (pre-selected, user can change on "Choose what to share") | off |
| `files` | ask | `ask`/`off` | ask | off |
| `handoff` | ask | `ask`/`off` | ask | off |
| `ring` | quiet | `on`/`off` | on | off (computers cannot ring) |

  `notifications`, `messages`, `calls`, `screen`, `input` keys are reserved
  and read as `off` in Phase 1; the UI hides their rows until their phase.
- Two-sided: a feature runs only if both sides allow it. The giving side's
  switch governs what leaves it; the receiving side's governs what it
  accepts. Each side sends its full policy on the `Policy` lane at link open
  and on change; the peer shows it read-only ("Off on Pixel 8"). Remote
  policy is display-only and never written to the local file.
- Ask approvals: one per offer, expire after 60 s unanswered (sender sees
  "No answer"), never remembered.

## Features (pinned)

- **Clipboard.** Text up to 1 MiB, images (PNG) up to 10 MiB; larger is
  skipped with "Too big to share" on the sender. Desktop watches the
  clipboard and sends on change while the feature is on. Android sends
  laptop→phone automatically; phone→laptop only by user action (Send
  clipboard button, notification action, quick-settings tile, share sheet).
  Items the OS marks sensitive (Android `ClipDescription.EXTRA_IS_SENSITIVE`;
  Linux `x-kde-passwordManagerHint` = `secret` MIME) are not sent ("Password
  skipped"). History: last 20 received items, local only, in memory plus an
  opt-in file; cleared on forget. Content never logged.
- **Files.** Offer `{ name, size, mime }` → receiver asks → transfer over a
  `File` lane in 1 MiB chunks with BLAKE3 per chunk, resumable within the
  same link session; saved to the OS Downloads dir with a non-clobbering
  name; names normalised and stripped of path separators. Interrupted
  transfers show "Device left" with Retry.
- **Handoff.** `{ kind: Url | File, … }`. URLs `http`/`https` only; opened
  with the default handler after the ask. Files go through the Files flow
  and open after saving.
- **Ring.** Laptop → phone only; phone plays the alarm stream at full
  volume for 30 s or until stopped on either device; works with the phone
  on silent (Android `USAGE_ALARM`).
- **Ping.** Diagnostic lane used by `carrova link status --ping` and tests.

## Pairing UX (pinned)

- Desktop: Nearby → "Add a device": a QR for phones and a typed code for
  computers (3-minute code like today). States per mockup: waiting,
  "<Phone|Computer> found" (key check), linked, code expired, couldn't
  connect.
- Key check (signed off): the found device's fingerprint is shown with
  Confirm / "Not my <phone|computer>"; nothing is stored until Confirm.
  The joining device shows its own fingerprint while it links, so the user
  can compare the two.
- Android: welcome → scan (camera permission) or type code → linking →
  linked/failed/not on Wi-Fi → "Choose what to share" (Phase 1 rows only)
  → home.
- After linking, the desktop device page shows the phone; the Android home
  shows the computer.

## Android shell (pinned)

- Same Tauri project (`apps/desktop`), Android target via `tauri android`,
  `minSdk` 29, `targetSdk` current stable. App id `org.byteveda.Carrova`.
- Kotlin foreground service `LinkService` (`connectedDevice` type) starts at
  first link and on boot if any link exists; runs the Rust supervisor;
  shows "Linked to <name>" with actions Send clipboard / Pause; holds a
  multicast lock only while browsing or advertising.
- Kotlin glue only for: clipboard, foreground service, multicast lock,
  quick-settings tile, share target, camera QR scan, ring (alarm audio),
  Downloads (MediaStore), open URL/file. Reached through `carrova-platform`
  trait methods via a JNI bridge owned by `carrova-platform`
  (`platform/android/`), no business logic in Kotlin.
- Phone UI: the React app renders the phone layouts from the Android
  mockup when `navigator` reports a mobile platform from the bridge
  (`about.os == Android`); same IPC contract.
- Distribution: signed APK attached to GitHub releases; the in-app update
  check reuses the desktop updater feed with an Android entry.

## CLI (pinned)

```
carrova devices                         # list known devices, linked state
carrova devices link <code|ticket>      # pair-only with a computer
carrova devices features <device> [<feature>=<on|off|ask> ...]
carrova link status [--ping]
carrova link serve                      # run the supervisor headless
carrova clip send --to <device>         # send the current clipboard
carrova file send <path> --to <device>
carrova handoff <url|path> --to <device>
```

All go through the one supervisor per user (app background listener or
`carrova link serve`), reached over `<state>/link.sock` (Unix socket,
`0600`, owner-checked; Windows named pipe with the user's SID ACL). If no
supervisor runs, the command starts one for its own duration.

## Acceptance (Phase 1 exit gate)

Automated (`just ci` green, output shown):
- ADR 0002 handshake: pair-only, link, bad tag, wrong key, replayed nonce,
  forgotten device, duplicate connection, older peer.
- Two supervisors in one process: link up on presence, down on forget,
  reconnect after a dropped connection, policy two-sided gating for each
  feature, ask expiry, lane refusal for unknown kind.
- Clipboard size limits and sensitive skip; file name normalisation and
  resume; Handoff URL scheme filter.

Real hardware (user):
- Ubuntu 24.04 laptop + Android 13+ phone: pair by QR; clipboard both ways;
  file both ways; Handoff a URL both ways; Ring.
- Link survives Wi-Fi drop and return, laptop suspend/resume, Android app
  swiped away, phone reboot.
- Idle phone battery over 8 h with one link: Carrova below 2% of total.
- Two Ubuntu computers: link by code, clipboard and file both ways.
