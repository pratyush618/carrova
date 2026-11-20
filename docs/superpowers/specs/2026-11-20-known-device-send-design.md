# Code-less send between trusted computers

Date: 2026-11-20. Status: draft, awaiting user review and the sign-off of
ADR 0004 (`docs/adr/0004-known-device-requests.md`, Status: Proposed).
Screens (source of truth): `docs/mockups/gui-ecosystem-desktop.html`, by
`screen:state` id: `sendto:choose`, `sendto:empty` (`add` at ~:1913);
`sendwait:waiting|reviewing|declined|timeout|away` (~:1931);
`sendreq:open|closed|locked|expired` (~:1935); `peer:linked|noreq`
(~:1537); `nearby:computer` (~:1473); `home:move` (~:1428, the in-app
card); `tray:request` (~:1824); `settings:nearby` (~:1820, section
`nearby`).
Parent: `docs/superpowers/specs/2026-11-12-background-nearby-design.md`
(goal 5) and plan `docs/superpowers/plans/2026-11-12-background-nearby.md`
Tasks 3.0–3.4 (`:255-297`). Where this document and the plan differ, this
one wins; differences are listed under "What changes in the plan".

## Why

Every move needs a pairing code, even between two computers that have moved
together before or are linked. Typing a code on both screens is the main
friction left. Two computers that already share a secret and know each
other's device key can prove that to each other; the code adds nothing for
them. The receiver must still decide what lands on it.

## Goals

1. A trusted computer sends a move without a code. Trusted = known (a
   code-paired move happened, ADR 0001) or linked (ADR 0002). Computers
   only.
2. The sender picks the computer; a request reaches it; the receiver gets a
   notice and chooses Review or Decline. Review opens today's receive
   review (dry-run plan → approve → staged apply → undo).
3. The receiver can turn requests off per device ("Send requests", on by
   default for computers).
4. CLI and desktop behave the same (boundary rule 2).

## Non-goals

- Phone → computer (or computer → phone) setup moves.
- Auto-accept, "quick save", trusted-device approval skipping.
- Code-less sends for unknown computers. They need a code once.
- Any change to what a move does after Review (plan, staging, backup, undo).
- Moves over the internet or through a relay.

## Anti-goals (failure even if it works)

- Anything written on the receiver before its user approves the plan.
- A stranger (or a forgotten, requests-off or phone device) making a notice,
  tray item, sound or screen wake appear.
- Device or app names on a locked screen without the user's opt-in.
- The request listener costing noticeable idle CPU or memory.

## Constraints

- Boundary rules (CLAUDE.md): OS calls only in `carrova-platform`
  (`wake_display` and `screen_lock` exist: `crates/carrova-platform/src/platform.rs:407-414`);
  `src-tauri` holds no business logic; services never depend on core.
- `carrova-crypto` and the transport handshake change → ADR 0004 needs
  explicit user sign-off before Task 3.1 starts.
- Never log secrets, tags, nonces, codes, full public keys.
- Tests outside `src/`; regression test first; `just ci` green.

## Approaches

| | Approach | Trade-offs |
|---|---|---|
| **A (chosen)** | New `Intent::Request { nonce, tag }` modelled on `Intent::Link` (`crates/carrova-transport/src/wire/messages.rs:74-99`), proved with the per-pair `KnownSecret`; reuses the nonce set, admission and close codes; after acceptance the existing send session runs unchanged. | One new intent, tag label and a few frames. Works for known and linked computers alike, with or without a running link. |
| B | Request as a message over the open link connection. | No handshake change, but only works while linked and connected; known-only computers still need a code; moves depend on link uptime and policy. |
| C | Plan Task 3.0's separate intent: `Request { from: Fingerprint, nonce, proof = HMAC(KnownSecret, binding ‖ nonce) }` plus an identity signature. | Redundant with the link machinery (more new code); `from` names the dialer to the LAN; fingerprints are display-only (ADR 0001 §2). |

## Decided (user, recorded)

- Approach A.
- Linked computers count as trusted for sending; phones never. This amends
  ADR 0002 ("pair-only, no moving"): linking a computer also lets it
  *request* a move; the receiver still approves (ADR 0004 §7).
- Request summary before acceptance = counts only `{ apps, bytes,
  has_sensitive }`. No names.
- Lifetime 2 minutes. Receiver can Decline; sender can Cancel.
- The receiver listens only while visibility ≠ No one.
- Per-device "Send requests" toggle on computer device pages, default on.
  Off → that device's requests are closed silently, exactly like an unknown
  device's.
- Admission: ≤ 3 pending requests, ≤ 5 attempts/min per source address,
  10 s handshake deadline, replay protection via the first-use nonce set.
- UI per the mockup. Button label "Send setup". Declined → one action
  "Change what moves". Timed out → "Send again".
- Unknown computers always need a code, in every visibility mode (this
  replaces the parent spec's "unless visibility is Everyone",
  `2026-11-12-background-nearby-design.md:124-126`).

## Findings from the code

1. **Known and linked use the same secret.** Both are a `DeviceRecord` plus
   a `devices/<hex>.key` `KnownSecret` (`crates/carrova-core/src/devices/mod.rs:1-24`,
   `devices/record.rs:12-36`); "linked" is only a policy file on top. One
   tag construction serves both. New label `carrova/1/known-request`
   (16-byte tag), domain-separated from discovery (`carrova/1/known-discovery`,
   8 bytes) and link (`carrova/1/known-link`) tags
   (`crates/carrova-crypto/src/pairing/mod.rs:125-129`), so a captured link
   or discovery tag never admits a request.
2. **Direction.** Today the move receiver dials the move sender's
   `Listener` (`crates/carrova-core/src/engine/receive.rs:57-68`,
   `engine/send/mod.rs:79-104`). For a request the receiver must listen and
   the sender dial: on the request connection the move sender is the
   handshake *receiver* (it sends `Hello`), as the dialer on a link is.
3. **The move cannot simply run on the request connection.** A
   `Keys::Link` connection has no resume secret
   (`crates/carrova-transport/src/handshake/keys.rs:64-72`), and resume
   needs the move sender to be the listener (`engine/send/pairing.rs:68-90`).
   Smallest correct option: the request connection hands the receiver a
   one-time pairing code and port (never shown, never advertised); the
   receiver dials the sender's existing `SendHandle` listener and the
   move is a normal code-paired move. The receiver pins the requester's key
   with the existing QR pin path (`handshake/secret.rs:103-109`,
   `handshake/receiver.rs:150-153`).
4. **A pinned move pairing skips the pin today.** The receiver accepts
   `Established` without `Identify` unless `must_identify()`
   (`handshake/secret.rs:96-101`, `handshake/receiver.rs:144`), which is
   false for move pairings. Must become true when a pin is set (regression
   test first).
5. **The link listener is the wrong place.** It runs only with the link
   supervisor (`crates/carrova-core/src/ecosystem/dial.rs:54-80`,
   `ecosystem/supervisor.rs:298`); known-only computers would never receive.
   Requests get their own `Listener` and presence TXT key `rp`.
6. **No per-minute limit exists.** `HandshakeSlots` caps concurrency only
   (`crates/carrova-transport/src/admission.rs:37-76`, `tuning.rs:72-74`).
7. **Name clash.** `apps/desktop/src-tauri/src/send_request.rs` already
   means "Send with Carrova" file paths. Name the new module
   `move_request`.
8. **Presence carries 13 tags, admission checks 64.** A sender finds the
   receiver only by a `kt` tag; `MAX_KNOWN_TAGS = 13`
   (`crates/carrova-transport/src/discovery/presence.rs:22-24`, filled in
   `crates/carrova-core/src/presence.rs:160-190`). See builder call 1.
9. **`resolve` takes only a fingerprint or full key**
   (`crates/carrova-core/src/devices/mod.rs:365-384`), no names. See
   builder call 4.
10. **Lock-screen copy in the mockup shows the device name**
   (`sendreq:locked`, "Desktop PC: 1 request"), while the brief and the
   parent spec say no names by default ("A known device wants to send you
   files."). Open question 2.

## Builder calls (main thread, user may revisit)

Made after a builder check of this draft. Each is a decision taken on the
main thread, not by the user; the user may overturn any of them.

1. **Tag limit.** While the request listener runs, the advert's 13 `kt`
   tags go first to computers with "Send requests" on (most recently used
   first), then to the other known devices as today. A trusted computer
   beyond those 13 cannot find this one: on its side `sendto:choose` shows
   it greyed as "Away" with "Use a code". The 64-candidate admission limit
   is unchanged. Advert budget with `rp`: fixed parts 283 + 9
   (`\nrp=65535`) = 292 bytes, plus a 200-byte name = 492 ≤ 512
   (`discovery/presence.rs:18-21`); update that comment and its test.
2. **Presence is owned by the caller, not by `listen_requests`.** CLI
   `carrova receive --requests` starts `Presence` itself, as
   `apps/cli/src/link.rs:314-322` does. The desktop app uses its
   `BackgroundPresence` (`apps/desktop/src-tauri/src/presence.rs:23-76`).
   The desktop request listener runs on an `Engine` with the app's real
   event sink, not `NullSink` (which `BackgroundPresence` uses,
   `presence.rs:57-61`), so request events reach the UI. The lock file
   covers the request listener only; if the app already listens, the CLI
   exits with "Requests are answered by the Carrova app on this computer."
3. **"No pairing advert", not "discovery off".** A requested send skips
   only `advertise_pairing` (nameplate) and gives its listener no
   `device_name`. Discovery stays on so the resume advert still runs and a
   dropped requested move resumes as today
   (`crates/carrova-core/src/engine/send/pairing.rs:68-90`; a resume
   connect has no address and finds the sender by its resume tag,
   `crates/carrova-transport/src/connect.rs:94-100`).
4. **Device names on the CLI.** `--to <DEVICE>` and
   `carrova devices requests <DEVICE>` accept a fingerprint, a full key, or
   an exact device name. A name matching more than one device is an error
   listing each match with its fingerprint. `KnownDevices::resolve` gains
   the name lookup (fingerprint and key keep priority).
5. **Screen ids.** All screens cited here exist in the mockup; they are
   listed by exact `screen:state` id in the header.

## Protocol flow

D = sender's computer (dials), L = receiver's computer (listens). Full
frame list and rules: ADR 0004 §2–5.

```text
D (user picked L in "Send to…")             L (visibility ≠ No one, request listener up)
  prepare send: capture, manifest,
  SendHandle listener, fresh code,
  no pairing advert (resume advert later)
  find L in presence: kt tag matches, rp port
  ── QUIC + TLS ─────────────────────────▶  admission: Retry, 5/min/IP, ≤3 pending
  Hello{Request{nonce, tag}, identify} ──▶  tag vs ≤64 candidates (computers,
                                            requests on), nonce first use
                                       ◀──  Welcome
                                       ◀──  Confirm(L)      (KnownSecret proof)
  Confirm(D) ──────────────────────────▶
                                       ◀──  Identify(L)     (DeviceLink sig)
  Identify(D) ─────────────────────────▶
                                       ◀──  Established
  Request{apps, bytes, has_sensitive} ─▶  notice: card / pop-up + tray + wake
  "Waiting for L · 2:00"                    "D wants to send you its setup"
                                            user: Review
                                       ◀──  Review
  Proceed{port, code} ─────────────────▶
  close Normal
  ◀── QUIC to D:port, Hello{Pair{nameplate}, identify} ── (today's move)
  SPAKE2 with code, Confirm ×2, Identify ×2 (L pins D's key, D pins L's)
  manifest → L: dry-run plan → user approves → staged apply → undo
```

Other endings on the request connection:

| Event | Close | Sender screen | Receiver |
|---|---|---|---|
| Receiver declines | L: `Declined` | `sendwait:declined` | notice removed |
| 2 min pass | D: `Normal` (D's clock) | `sendwait:timeout` | `sendreq:expired` banner |
| Sender cancels | D: `Cancelled` | flow closes | notice removed silently |
| Either side vanishes | idle timeout (≤ 30 s) | `sendwait:away` | notice removed |
| Dial fails, no `rp`, refused (busy, rate) | none | `sendwait:away` | nothing |
| Not trusted there (forgotten, requests off) | L: `LinkFailed` | open question 3 | nothing |
| Newer request from same D | L: `Superseded` (old one) | n/a (D started the new one) | old notice replaced |

## Core API (adjusts plan Task 3.2)

Store keys are full public keys (`DeviceId`), not fingerprints (ADR 0001 Q8).

```rust
// sender
impl Exporter {
    /// As `prepare_send`, but no pairing advert (no nameplate, no name;
    /// the resume advert still runs) and a device key is required.
    pub fn prepare_request(&mut self) -> Result<SendHandle>;
}
impl SendHandle {
    /// Find `to` nearby, send the request, block until Review.
    /// Ok(()) → call `run` as today. Errors: RequestDeclined,
    /// RequestExpired, RequestRefused (not trusted there), PeerUnreachable,
    /// Cancelled. Callable again for "Send again" (fresh code each time).
    pub fn request_send(&mut self, to: &DeviceId) -> Result<()>;
}

// receiver
impl Engine {
    /// Bind the request listener and save `rp` for the advert. Does not
    /// start Presence (the caller owns it, builder call 2). Events go to
    /// this engine's sink. Fails with CoreError::NotVisible while
    /// visibility is No one.
    pub fn listen_requests(&self, cancel: Cancel) -> Result<RequestInbox>;
}
impl RequestInbox {
    pub fn next(&self, wait: Duration) -> Result<Option<PendingRequest>>;
}
pub struct PendingRequest { /* id, from: DeviceId, name (local record),
                                summary: RequestSummary, expires_at */ }
impl PendingRequest {
    /// Send Review, take Proceed, connect pinned → today's Importer.
    pub fn review(self, cancel: Cancel) -> Result<Importer>;
    pub fn decline(self);
}
impl KnownDevices {
    pub fn set_send_requests(&self, p: &dyn Platform, id: &DeviceId, on: bool) -> Result<()>;
}
```

- `PendingRequest` emits `Event::RequestEnded { id, why: Cancelled | Expired
  | Superseded | Lost }` through the engine's sink when the connection ends
  before an answer.
- The request `KnownPeers` impl lives in core: computers with
  `send_requests` on, ordered by `known_by_recent_use`
  (`crates/carrova-core/src/lan/nearby.rs:119-135`).
- `rp` is saved/removed like the link port (`crates/carrova-core/src/presence.rs:127-147`)
  and added to `advert` (`presence.rs:160-190`), which also orders tags
  requests-on computers first while `rp` is set (builder call 1).
- `KnownDevices::resolve` also matches an exact device name; several
  matches → `CoreError::AmbiguousDevice` carrying each match's name and
  fingerprint (builder call 4).
- Receiving after `review` is today's `plan_for_review` / approve path,
  unchanged.

## Desktop (Task 3.3), mapped to mockup states

Sender:

| State | When |
|---|---|
| `sendto:choose` | After Pick. "No code needed": known/linked computers (local records, `OsKind` ≠ Android) whose advert carries a tag for us and `rp`; others greyed as "Away" (Wake only with a hint, else "Use a code"; includes computers past the receiver's 13-tag limit). "Needs a code once": new computers nearby, "Use a code". |
| `sendto:empty` | No trusted computers: "Send with a code". |
| `sendwait:waiting` | `request_send` running; countdown from 2:00; Cancel. |
| `sendwait:reviewing` | After `Proceed`, once the receiver connects (`PeerConnected`); Cancel move / Close (tray carries on). |
| `sendwait:declined` | `RequestDeclined`; actions Close, "Change what moves". |
| `sendwait:timeout` | `RequestExpired`; actions Close, "Send again". |
| `sendwait:away` | `PeerUnreachable`; Close, "Try again". |

"Send setup" on `peer:linked` and `nearby:computer` opens `sendto:choose`
(device page) or goes straight to `sendwait:waiting` (Nearby card, device
already chosen), as in the mockup.

Receiver:

| State | Surface |
|---|---|
| `sendreq:open` | In-app card on Home (`home:move`): name, counts, time left, Decline / Review. |
| `nearby:computer` | Card for a trusted computer: "Send setup" goes to `sendwait:waiting`. |
| `tray:request` | Request item first in the tray menu: Decline / Review. |
| `sendreq:closed` | OS notification (Decline / Review best effort; clicking opens the app on the card), tray item at the top (`tray:request`), `wake_display` best effort. |
| `sendreq:locked` | `screen_lock` = locked or unknown → notification without counts and, unless the lock-screen setting allows, without the device name. |
| `sendreq:expired` | Home banner "A move request from … expired", Dismiss. |
| `peer:noreq` | "Send requests" toggle off; copy "Off: … needs a code to send you a move." |
| `settings:nearby` | Footnote: requests arrive only while visible to Known devices or Everyone. |

Bridge (`crates/carrova-view/src/ipc/mod.rs`): event
`BridgeEvent::MoveRequestArrived { request: RequestView }` and
`MoveRequestEnded { id, why }`; commands `requestSend(session, to: DeviceIdHex)`,
`answerRequest(id, 'review' | 'decline')`, `setSendRequests(device, on)`.
`src-tauri/src/move_request.rs` only relays (no policy). The request
listener runs beside `BackgroundPresence`
(`apps/desktop/src-tauri/src/presence.rs:23-76`), follows visibility, and
uses an `Engine` with the app's event sink (not `NullSink`).

## CLI (parity, open question 4)

- `carrova send --to <DEVICE>`: device = fingerprint, full key or exact
  name, resolved to exactly one known computer (`KnownDevices::resolve`,
  `crates/carrova-core/src/devices/mod.rs:365-384`, extended with names;
  an ambiguous name lists the matches with fingerprints); conflicts with
  `--resume`. Prints the counts and "Waiting for <name> (expires in 2:00)",
  then today's send output. Declined / expired / unreachable / refused end
  with a message and exit code 1. Ctrl-C cancels (`Cancelled`). `--json`
  adds `{"request": "sent" | "review" | "declined" | "expired" | …}` lines.
- `carrova receive --requests`: foreground listener while visibility ≠ No
  one (else an error naming `carrova visibility`). Shows each request
  (name, counts, time left) and asks Review / Decline on the terminal;
  Review continues as `carrova receive` does. Needs a terminal: `--yes`
  never answers a request (no auto-accept).
  It starts `Presence` itself while it listens (builder call 2).
- `carrova devices requests <DEVICE> on|off` (same device lookup).
- One request listener per user: a lock file in the state dir covers the
  request listener only; if the app already listens, the CLI exits with a
  clear message.

## Tests

Regression first; the plan's tests kept and extended.

Crypto (`crates/carrova-crypto/tests/pairing/known/`):
- `request_tag_is_not_a_link_or_discovery_tag` (all three directions),
  `request_tag_binds_dialer_key`, `request_tag_debug_redacted`.

Transport (Task 3.1, `crates/carrova-transport/tests/request/`,
`tests/admission/unit.rs`, `tests/wire/…`):
- `known_request_is_admitted`, `unknown_device_closed_without_reply` (no
  `Reject`, close `LinkFailed`, no `IncomingRequest`),
  `link_hello_not_admitted_as_request`, `replayed_request_rejected`,
  `sixth_attempt_in_a_minute_rejected`, `fourth_pending_request_refused`,
  `newer_request_from_same_device_supersedes`,
  `slow_handshake_closed_after_10s`, `wrong_identity_signature_rejected`,
  `wrong_known_secret_rejected`, `request_without_identify_rejected`,
  `decline_closes_both_sides_cleanly` (`Declined`),
  `sender_cancel_reaches_listener`, `summary_only_after_established`,
  `proceed_code_never_in_debug`, `pinned_move_pairing_requires_identify`,
  `pinned_move_pairing_rejects_other_key` (`KeyMismatch`),
  `old_intent_type_rejects_request_hello` (fixture),
  `advert_without_rp_parses`, `broadcast_reply_with_rp_fits_512`,
  `request_messages_never_panic` (proptest, 10k cases).

Core (Task 3.2, `crates/carrova-core/tests/request/`, two engines in one
process):
- `request_then_review_moves_files`, `decline_leaves_receiver_untouched`,
  `request_times_out_after_two_minutes` (fake clock),
  `nothing_written_before_approval`, `sensitive_items_still_asked_after_request`,
  `requests_off_device_closed_silently`, `phone_is_never_a_candidate`,
  `forgotten_device_closed_silently`, `no_listener_while_visibility_no_one`,
  `rp_advertised_only_while_listening`,
  `requested_send_skips_pairing_advert` (no nameplate, no name),
  `dropped_requested_move_resumes` (resume advert still runs),
  `burned_code_ends_request_send`, `requested_move_remembers_like_code_move`,
  `send_again_uses_fresh_code`,
  `request_tags_put_requests_on_computers_first` (14+ known devices: the
  13 tags hold every requests-on computer first),
  `computer_past_tag_limit_shows_away` (sender side),
  `resolve_by_exact_name`, `ambiguous_name_lists_fingerprints`,
  `request_events_reach_engine_sink`.

Desktop (Task 3.3): plan's list, plus `declined offers only Change what
moves`, `timeout offers Send again`, `noreq toggle persists`,
`locked notice hides counts and name by default`, `tray item is first`,
`window closed wakes display (fake platform)`; e2e `known-send.spec.ts` on
the fake port (happy path, decline).

CLI: `send_to_unknown_device_errors`, `send_to_phone_refused`,
`send_to_by_name`, `send_to_ambiguous_name_lists_matches`,
`receive_requests_needs_visibility`, `receive_requests_needs_terminal`,
`receive_requests_starts_presence`, `receive_requests_exits_when_app_listens`,
`devices_requests_toggle`, `devices_requests_by_name`.

Transport additions: `broadcast_reply_with_rp_fits_512` (longest name,
13 tags, `lp` and `rp`).

E2E (Task 3.4): `just e2e --lan` two containers: code move A → B (both
remember), then `carrova send --to B` on A while B runs
`carrova receive --requests`, Review, approve, verify files and that no
code was typed; then a decline run; then requests off on B → A fails with
"refused" and B shows nothing.

## Open questions for the user

1. **Move after Review = code-paired move with a one-time code sent inside
   the authenticated request** (ADR 0004 §4), not the request connection
   itself. Recommendation: accept; the alternative needs new crypto for a
   resume secret.
2. **Lock screen.** The mockup shows "Desktop PC: 1 request"; the brief and
   parent spec say no names by default. Recommendation: by default "A
   trusted computer: 1 request", name only with "Show names on the lock
   screen"; update the mockup copy.
3. **What the sender sees when the receiver silently refuses** (forgotten,
   requests off). The mockup has no state for it. Recommendation: one
   ending "<name> needs a code now" with "Use a code" (does not say why),
   added to `sendwait`.
4. **CLI surface** as above (`send --to`, `receive --requests`,
   `devices requests`), and no headless background listener in v1 (the
   desktop app covers window-closed). Recommendation: accept.
5. **One pending request per device** (newer replaces older).
   Recommendation: accept.
6. **Requests while a receive is already running.** Recommendation: still
   notify; Review is disabled with "Finish the current move first" until it
   ends or the request expires.
7. ADR 0004 open questions 2, 3 and 5 (reuse link proofs, reuse
   `LinkFailed`, own listener and `rp`). Recommendation: accept.
8. **ADR 0002 amendment** (linking a computer also allows move requests;
   ADR 0004 §7). Recommendation: accept as worded.

Not open questions: the five builder calls above stand unless the user
overturns them.

## What changes in the plan

- Task 3.0: ADR 0004 replaces the sketched `Request { from: Fingerprint,
  nonce, proof }`.
- Task 3.1: `KnownLookup` → existing `KnownPeers`; `admission.rs` is
  extended (per-minute budget, pending cap), not created;
  `IncomingRequest::accept -> Authenticated` → `review() -> Proceed`;
  `connect(Target::Known{fingerprint…})` → a request call keyed by
  `DevicePublic`; plus pinned move pairing and `rp`.
- Task 3.2: `Exporter::request_send(&Fingerprint)` →
  `Exporter::prepare_request` + `SendHandle::request_send(&DeviceId)`;
  `receive_request.rs` builds an `Importer` through the pinned code path.
- Task 3.3: event `SendRequestArrived` → `MoveRequestArrived`; module
  `move_request` (not `requests`/`send_request`).

## Left to the builder

- Exact type and module names inside each crate; how `RequestInbox`
  threads are spawned; how the listener restarts on visibility changes.
- The lock-file mechanism for the request listener (one per user).
- Copy strings beyond those fixed in the mockup.
- Whether the sender's `Expected::Pair` pin is a field or a new variant.
- Retry/backoff when admission refuses (busy) before reporting "away".
