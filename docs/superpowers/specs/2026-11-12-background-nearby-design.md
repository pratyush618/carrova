# Background mode, nearby devices and the quiet-instrument UI

Date: 2026-11-12. Status: design approved in mockups, awaiting plan review.
Mockup (source of truth for every screen): `docs/mockups/gui-v4.html`.

## Why

Carrova only works while its window is open, and both computers must type a
code every time. The user wants Carrova to behave like a premium utility:
it waits quietly in the background, tells you when another computer wants to
send you something (even if the screen is off), shows the computers around
you, and lets computers that have moved together before skip the code. While
doing this, the whole app moves to one calm, consistent look ("quiet
instrument") where nothing is said twice.

## Goals

1. **Background mode.** Closing the window keeps Carrova running with a tray
   icon; opening the window removes the icon. A setting turns this off
   (closing then quits). Optional start at login (starts closed, in the tray).
   Idle cost in the background is minimal: the webview is destroyed while
   closed and only a small Rust listener stays.
2. **Notifications.** Three surfaces for the same events: system pop-up,
   screen wake (send requests from known devices only), and an in-app bell
   that keeps everything for 30 days. Users choose per event kind
   "Pop-up and bell" or "Bell only", can pause all alerts (1 hour / until
   tomorrow), and choose whether names show on the lock screen (default off:
   "A known device wants to send you files.").
3. **Nearby devices.** A Nearby page (map and list) of Carrova computers on
   the network: known (tinted, "Send", no code), new ("Send with code"), away
   (faded, "Forget"). Visibility shows once, as a pill on Nearby:
   No one / Known devices / Everyone for 10 minutes (reverts by itself).
4. **Known devices.** After the first code-paired move, both computers
   remember each other (setting "Remember devices I move with", default on).
   Identity is a per-install key, never the MAC address. Known devices can be
   renamed and forgotten, one or all. The MAC address may be saved only as a
   Wake-on-LAN hint (setting, default off).
5. **Code-less send.** A known device can ask to send; the receiver gets the
   request (pop-up, tray, bell, screen wake), chooses Review or Decline, and
   the existing review/plan/approve flow runs unchanged. New devices still
   need the code once.
6. **Quiet-instrument UI** for Home, Nearby, History, Apps, Settings, Send,
   Receive as in the mockup.

## Non-goals

- No cloud relay, accounts or contacts. LAN only, as today.
- No auto-accept: every incoming move is reviewed. ("Quick save" is out.)
- No change to what a move does once approved (staging, backup, undo).
- No Flathub / macOS notarisation work (deferred, unchanged).
- Wake-on-LAN sending is optional polish, not required to ship goal 4.

## Anti-goals (would make this a failure even if it works)

- A stranger on the network can make a notification appear or start a
  transfer without the user choosing "Everyone for 10 minutes".
- Names or file names shown on a locked screen without opt-in.
- The background listener costs noticeable CPU, memory or battery.
- Anything said twice on one screen (sidebar label repeated as a title,
  status shown in two places, a count repeated in a link).
- Settings that disagree with each other (the Nearby pill and Settings →
  Nearby must bind one value).

## Constraints (from the project rules)

- OS-specific code only in `carrova-platform` (tray-host detection, screen
  wake, autostart, MAC lookup). `src-tauri` holds no business logic; CLI
  and UI behave identically where both apply (`carrova devices`,
  `carrova visibility`, `carrova send --to <device>`).
- Any change to `carrova-crypto` or the transport handshake needs explicit
  user sign-off. Phases 2 and 3 each start with a written ADR and a sign-off
  gate.
- Never log keys, secrets, pairing codes, device keys or MAC addresses.
- Tests outside `src/`, bug fix → regression test first, `just ci` green.
- Sensitive items still default off and are asked one by one.

## UI decisions (as approved in the mockup)

- **Shell.** Sidebar: Home, Nearby (green dot when visible), History (count),
  Apps, Settings; this computer's name at the foot. A 52 px header bar on the
  right holds only page tools (search, filters, view switch) and the bell.
  Hub pages show no title: the selected sidebar item is the title. Flows
  (Send, Receive) take the whole window with their own bar: back arrow (←, returns to the previous screen), flow name,
  step labels, from → to chip, window controls.
- **Bell** opens a popover over the current page (never navigates); closes on
  a second click, outside click, or any navigation.
- **Settings** swaps the sidebar for its sections (back arrow, search,
  General, Sending, Nearby & devices, Notifications, Moves, Privacy, Updates,
  About). Sub-settings are indented and dim when their parent is off. Search
  shows matches across sections with the hit highlighted.
- **Nearby map**: this computer centred (white laptop icon on accent disc),
  devices as pills on rings ("answering now" inner ring); one pulse leaves
  this computer when a scan starts (none under reduced motion). Selecting a
  device opens a card (last move, moves together, sends without a code,
  primary "Send to …").
- **Apps** and **History**: master–detail; category/day headings stick while
  scrolling; rows state exceptions only ("Not here", "Interrupted",
  "Undone", "Undo expired", "Community"). Sidebar label is "Apps".
- **Send → Pick** at scale: summary line, sticky search + filter bar
  (All / Ticked / Changed by you / Largest), keys pinned open, every other
  category collapsed to one line, opened category shows 8 rows then
  "Show N more". Lists over 200 rows are virtualised.
- **Send steps**: Pick → Choose computer → Their review → Move.
  **Receive steps**: Request → Review → Move → Done; Receive opens on
  "Ready to receive" with a code box for unknown senders.
- **Desktop**: tray icon only while the window is closed; tray menu has the
  waiting request (Review / Decline) first, then Open, Send files…, Nearby,
  Pause alerts for 1 hour, Visible to ›, Quit.

## Technical calls made in brainstorming (builder may revisit with reason)

- Device identity: Ed25519 key per install (`ed25519-dalek`, already a
  workspace dependency), stored 0600 in the state dir; displayed as a short
  fingerprint ("7F3A·19C2").
- Known-device trust is established only inside an already PAKE-authenticated
  session (first code pairing), by exchanging identity public keys signed
  over the channel binding. Later connections authenticate by signing the
  new channel binding with the identity key; the peer must be in the known
  list.
- Known-device discovery tags reuse the resume-tag idea: a fresh nonce per
  advert plus an HMAC keyed by a per-pair secret, so only known peers can
  recognise an advert and adverts can't be linked.
- Direction: today the receiver dials. For code-less send the receiver
  listens (only when visibility is Known or Everyone), the sender dials and
  sends a request; admission is rate-limited and unknown peers are rejected
  before any UI appears unless visibility is Everyone.
- Screen wake: Linux `org.freedesktop.ScreenSaver.SimulateUserActivity`
  (best effort; GNOME also wakes for critical notifications), macOS
  `IOPMAssertionDeclareUserActivity`, Windows `SetThreadExecutionState`
  with `ES_DISPLAY_REQUIRED` (best effort, documented as such).
- Tray: Tauri `tray-icon` feature; on Linux without a StatusNotifier host
  (plain GNOME) closing minimises instead of hiding, and Settings explains
  why. The webview is destroyed on close and recreated on open; the UI
  re-fetches state on mount.
- Notifications: `tauri-plugin-notification` for pop-ups; actions are not
  relied on (Review opens Carrova's own screen).
