# ADR 0004: Code-less send requests between trusted computers

- Status: Proposed (awaiting user sign-off)
- Date: 2026-11-20
- Scope: Phase 3 of `docs/superpowers/plans/2026-11-12-background-nearby.md`
  (Tasks 3.0–3.4). Design: `docs/superpowers/specs/2026-11-20-known-device-send-design.md`.
  This ADR is the Task 3.0 gate. It supersedes the protocol sketched in
  plan Task 3.0/3.1 where they differ (listed under Consequences).
- Builds on: ADR 0001 (device key, `KnownSecret`, known store), ADR 0002
  (`Intent::Link`, link tag, link proofs, `KnownPeers`, `LinkFailed`).
- Amends: ADR 0002 ("pair-only, no moving"). See Decision 7.
- Touches: `carrova-crypto` (one new tag label and type) and the transport
  handshake (new intent, three frames, one close code, one TXT key, a
  pinned move pairing, a rate limit). Both need explicit sign-off
  (CLAUDE.md, "Crypto").

## Context

A trusted computer should be able to send a move without a pairing code.
"Trusted" = known (code-paired move done before, ADR 0001) or linked
(ADR 0002), computers only. The receiver still reviews the plan and
approves; nothing is written before that.

What exists:

- `Hello { proto, intent, identify, purpose }`, both later fields
  `#[serde(default)]` (`crates/carrova-transport/src/wire/messages.rs:13-27`).
  `Intent` is `Pair { nameplate }`, `Resume { nonce, tag }`,
  `Link { nonce: [u8; 16], tag: [u8; 16] }` (`wire/messages.rs:74-99`).
  Frames after `Established` already exist for pair-only (`PeerInfo`,
  `Linked`, `wire/messages.rs:47-59`).
- Roles: the side that sends `Hello` is the handshake receiver
  (`PakeRole::Receiver`), the listener is the handshake sender
  (`handshake/mod.rs:3-18`). For a move today the move receiver dials the
  move sender's `Listener` (`crates/carrova-core/src/engine/receive.rs:57-68`,
  `crates/carrova-core/src/engine/send/mod.rs:79-104`).
- The link tag is `HMAC(KnownSecret, "carrova/1/known-link" ‖ nonce ‖
  dialer DevicePublic)[..16]` (`crates/carrova-crypto/src/pairing/known.rs:75-93`,
  label `pairing/mod.rs:127`, `LINK_TAG_LEN = 16` at `pairing/mod.rs:99`).
  `KnownPurpose` has only `Discovery` (`known.rs:134-149`). `LinkTag` has
  redacted `Debug` and no `PartialEq` (`known.rs:177-188`).
- Link proofs: `KnownSecret::prove/verify` = `HMAC(KnownSecret,
  "carrova/1/link-confirm" ‖ role ‖ ChannelBinding)` (`known.rs:95-109`,
  `pairing/mod.rs:129`), then Ed25519 under `SignContext::DeviceLink`
  (`crates/carrova-crypto/src/identity.rs:96-122`). The transport reaches
  both through `Keys::Link` (`handshake/keys.rs:15-72`); a link has no
  resume secret (`keys.rs:64-72`).
- Admission: `Expected::admit` recognises a link `Hello` by checking the
  tag against the first `MAX_LINK_CANDIDATES = 64` known peers, all
  checked with no early exit, and admits its nonce once
  (`handshake/secret.rs:152-180`, `secret.rs:223-238`, `tuning.rs:81`).
  Nonces live in `ResumeNonces`, a bounded FIFO set per `Listener`,
  capacity 1024 (`crates/carrova-transport/src/replay.rs:111-145`,
  `listener.rs:96,130`, `tuning.rs:77`). An unrecognised link `Hello` gets
  no `Reject` frame and is closed `LinkFailed` (`handshake/sender.rs:108-118`,
  `secret.rs:195-201`, `receiver.rs:33-38`).
- `HandshakeSlots` gives a QUIC Retry to unvalidated addresses, then at
  most `MAX_PENDING_HANDSHAKES = 4` concurrent handshakes and
  `MAX_PENDING_PER_ADDRESS = 1` per IP (`admission.rs:37-76`,
  `tuning.rs:72-74`). There is no per-minute limit.
- Deadlines: `PRE_PAKE_TIMEOUT = 2 s` to stake, then `HANDSHAKE_TIMEOUT =
  10 s` (`tuning.rs:60,69`, `handshake/sender.rs:37-68`). Session idle
  timeout 30 s with a 5 s keep-alive (`tuning.rs:39-41`).
- The sender rejects any `proto` ≠ `PROTOCOL_VERSION = 1` before reading
  the intent (`handshake/sender.rs:101-107`, `lib.rs:82`). An older peer
  fails to decode an unknown `Intent` variant and drops that connection
  only (ADR 0002, Alternatives).
- Close codes 0–6 (`Normal` … `Superseded`); unknown codes decode as
  `None` (`wire/close.rs:7-50`).
- Known and linked devices are the same store: one `DeviceRecord` and one
  `devices/<hex>.key` (`KnownSecret`) per device, whether it got there by a
  code-paired move or by pair-only linking
  (`crates/carrova-core/src/devices/mod.rs:1-24`,
  `devices/record.rs:12-36`, `devices/mod.rs:221-246`). "Linked" is only a
  policy on top (`devices/policy/<hex>.json`,
  `crates/carrova-link/src/policy.rs:86-94`). **So a known computer and a
  linked computer hold the same kind of secret; one construction covers
  both.**
- A code pairing with a pinned key: `Secret::Pair { pin }` makes the
  receiver check the sender's `Identify` key before revealing its own
  (`handshake/secret.rs:25-32,103-109`, `handshake/receiver.rs:150-153`).
  Today only pair-only sets a pin (`connect.rs:81,146-158`). The receiver
  accepts `Established` without `Identify` unless `must_identify()`, which
  is true only for links and pair-only (`secret.rs:96-101`,
  `receiver.rs:144`): a pinned **move** pairing would silently skip the
  pin.
- Presence TXT `lp` carries the link listener's port while the link
  supervisor runs (`lib.rs:105-107`, `discovery/presence.rs:107-116,129,143`,
  `crates/carrova-core/src/presence.rs:127-147`,
  `crates/carrova-core/src/ecosystem/supervisor.rs:298`). The broadcast
  reply budget is 512 bytes with fixed parts ≤ 283 bytes and names ≤ 200
  bytes (`discovery/presence.rs:18-21`).

No new dependency is needed.

## Decision

Names: **dialer** = the computer that wants to send (move sender);
**listener** = the computer asked (move receiver). On the request
connection the dialer is the handshake receiver (`PakeRole::Receiver`) and
the listener is the handshake sender (`PakeRole::Sender`), as on a link.

### 1. Request intent and tag

- `Intent::Request { nonce: [u8; 16], tag: [u8; 16] }`, appended to
  `Intent`.
- `tag = HMAC-SHA256(KnownSecret, "carrova/1/known-request" ‖ nonce ‖
  dialer's DevicePublic)[..16]`, fresh random nonce per attempt. Same
  construction as the link tag; `KnownSecret` gains `request_tag` /
  `recognises_request`, and a type `RequestTag([u8; 16])` (redacted
  `Debug`, no `PartialEq`), like `LinkTag`.
- **Own label, not the link label.** The tag is checked before any proof
  and is replayable across connections. With the link label, a link
  `Hello` captured on the LAN would pass a request listener's admission (it
  has its own nonce set, see 3) and cost it a slot and the proof work.
  With its own label a link tag never admits a request and the reverse; a
  discovery tag (8 bytes, `carrova/1/known-discovery`) cannot either (other
  label and length). This is what ADR 0001 §5 reserved ("Phase 3 can add a
  request purpose with its own label").
- Length 16 for the same reason as the link tag (ADR 0002 §3): it is an
  online admission check against up to 64 secrets.

### 2. Handshake and proofs

```text
D → L  Hello { proto: 1, intent: Request { nonce, tag }, identify: true }
L → D  Welcome
L → D  Confirm(KnownSecret proof, listener)
D → L  Confirm(KnownSecret proof, dialer)           (only after L's verified)
L → D  Identify(listener, DeviceLink sig)
D → L  Identify(dialer, DeviceLink sig)             (only after L's verified)
L → D  Established
D → L  Request { apps: u32, bytes: u64, has_sensitive: bool }
       ... listener shows the notice; at most 2 minutes ...
L → D  Review                                        (user chose Review)
D → L  Proceed { port: u16, code }                   then D closes Normal
   or  L closes Declined                             (user chose Decline)
   or  D closes Cancelled                            (sender cancelled)
   or  D closes Normal at 2:00                       (expired, sender's clock)
```

- Proofs reuse the link's: `Keys::Link` (`KnownSecret::prove/verify`,
  label `carrova/1/link-confirm`) and `SignContext::DeviceLink`. Both are
  bound to this connection's `ChannelBinding` and role, so a proof cannot
  move to another connection; the intent each side read decides what the
  connection is for. New proof labels would add code and no protection.
- Each side requires the peer's `Identify` key to be the expected one: the
  dialer's stored key for the target; on the listener, the candidate whose
  request tag matched (as links, `keys.rs:57-62`).
- A request `Hello` without `identify: true` is not admitted.
- `Request` is read within the same 10 s handshake deadline. Only after it
  arrives does the listener tell its user anything. Its three fields are
  fixed size; `apps` and `bytes` are display hints only (Threat model).
- `Proceed.code` is a fresh one-time `PairingCode` (see 4), carried in a
  type with redacted `Debug` so a logged `Handshake` never prints it.
- New frames: `Handshake::{Request(RequestSummary), Review, Proceed { port,
  code }}`. Frames after `Established` are safe: both sides know from the
  intent that no session runs on this connection (as pair-only, ADR 0002
  §1).
- The connection uses the session transport (5 s keep-alive, 30 s idle
  timeout), so a dialer or listener that vanishes is noticed within 30 s.

### 3. Listener and admission

- A separate `Listener` (own QUIC endpoint and port, own `ResumeNonces`),
  `Listener::accept_request(known: Arc<dyn KnownPeers>, timeout)`. It runs
  only while visibility ≠ No one, and its port is advertised in a new
  presence TXT key `rp` (`TXT_REQUEST_PORT`) only while it runs.
  `PresenceAdvert` and `Nearby` gain `request_port: Option<u16>`.
  `rp=65535` adds at most 9 bytes (`\nrp=65535` in a broadcast reply, one
  length byte + 8 in TXT): fixed parts become ≤ 283 + 9 = 292 bytes
  (`discovery/presence.rs:18-21`), with a 200-byte name ≤ 492 of 512.
  The `MAX_NAME_BYTES` doc comment and its test must be updated to the new
  fixed size.
- **Tag limit (builder call, main thread; user may revisit).** A sender
  finds the listener only through a `kt` tag, and an advert carries at most
  `MAX_KNOWN_TAGS = 13` (`discovery/presence.rs:22-24`; filled by
  `known_by_recent_use` in `crates/carrova-core/src/presence.rs:160-190`),
  while the listener admits up to 64 candidates. While the request listener
  runs, the 13 tags go first to computers with "Send requests" on (most
  recently used first), then to the rest (as today). A trusted computer
  beyond the 13 cannot find this one: its "Send to…" list shows it greyed
  as "Away" with "Use a code". The 64-candidate admission limit stays (it
  bounds HMAC work, not discovery).
- Not the link listener: the link supervisor runs only where linking is
  used, and a known-only computer must still receive requests. Coupling
  requests to the link listener is what Approach B was rejected for.
- Candidates: the receiver's `KnownPeers` for requests returns only
  computers (`OsKind` not `Android`) whose per-device "Send requests"
  setting is on, most recently used first, first 64 checked (as links). A
  device that is a phone, forgotten, or has requests off is therefore not a
  candidate: its `Hello` fails the tag check and is closed exactly like a
  stranger's (`LinkFailed`, no `Reject`, no notice).
- Admission, before any HMAC:
  - QUIC Retry for unvalidated addresses (unchanged).
  - **5 admitted attempts per source IP per rolling minute**; the 6th is
    refused at once (`HandshakeSlots` gains an optional per-minute budget,
    used by this listener only).
  - **3 pending requests** (authenticated, waiting for an answer): while 3
    are pending, new connections are refused at admission
    (`Refusal::Full`). In-flight handshakes keep their own cap of 4 and 1
    per IP.
  - One pending request per device: a newer authenticated request from the
    same key replaces the older, which is closed `Superseded`.
- Deadlines: 2 s to stake, 10 s handshake including `Request` (existing
  constants). The listener drops a pending request it has not answered by
  2 min + 5 s grace (the dialer's clock started first and normally ends
  it).
- Replay: a recognised request nonce is admitted once per listener
  (`ResumeNonces::first_use`); unrecognised ones are not recorded. A
  listener restart forgets nonces; a replay then fails the proofs (bound to
  the new channel).
- Any failure (no matching tag, replayed nonce, bad proof, bad signature,
  wrong key) closes with **`LinkFailed`** (reused: it already means "known-
  device proof failed, no detail"). The local log records the kind only.

### 4. After Review: the move is today's code-paired move

- The dialer prepares its send as today (`SendHandle`, capture, manifest,
  `Listener` with a fresh `PairingCode`), but **without the pairing
  advert** (builder call, main thread; user may revisit): only
  `advertise_pairing` is skipped (nameplate, and the listener gets no
  `device_name`, so no name either). Discovery itself stays on, so the
  resume advert still runs and a dropped requested move resumes as today
  (`engine/send/pairing.rs:68-90`; a resume connect has no address and
  finds the sender by its resume tag, `connect.rs:94-100`).
- On `Review` the dialer sends `Proceed { port, code }` (its `SendHandle`
  listener port and code) and closes `Normal`. The listener dials
  `<request connection's remote IPv4>:<port>` with `Target::Pairing` and
  **pins** the requester's key: the move sender's `Identify` must carry the
  same `DevicePublic` that proved the request, checked before the
  receiver's own `Identify` is sent (existing QR pin path, ADR 0002 §2).
- From there everything is the existing session: SPAKE2 with the code,
  resume secret, `Identify`, manifest, dry-run plan, approve, staged apply,
  undo, `remember_move` (which replaces the `KnownSecret`, latest pairing
  wins, ADR 0001 §4).
- Required transport fixes for this: a pinned `Secret::Pair` must identify
  (`must_identify()` true when a pin is set), and the dialer's
  `Expected::Pair` gets the same optional pin so it checks the move
  receiver's `Identify` against the requested device. Both sides need a
  device key for a requested move regardless of the "Remember devices I
  move with" switch (like pair-only).
- Why a fresh connection and not the request connection: a move needs a
  resume secret for reconnects, and a `Keys::Link` connection has none
  (`keys.rs:64-72`); deriving one from `KnownSecret` is new crypto. The
  move sender also has to be the listener for resume
  (`engine/send/pairing.rs:68-90`). A one-time code delivered over an
  already mutually authenticated channel reuses all of that unchanged.

### 5. Close codes

- New `CloseCode::Declined` = 7 (appended; older peers decode it as
  unknown). Used only by the request listener when its user declines.
- Reused: `LinkFailed` (any failed proof), `Cancelled` (dialer's user
  cancelled), `Superseded` (newer request from the same device), `Normal`
  (`Proceed` sent, or expired on the dialer's clock).

### 6. Compatibility

- `PROTOCOL_VERSION` stays 1. `Intent::Request` is new: an older Carrova
  never runs a request listener nor advertises `rp`, so a new dialer never
  finds one to ask; the UI shows such a computer under "Needs a code once".
  If an old listener ever receives the intent, it fails to decode it and
  drops only that connection.
- Must be proved by tests: old `Hello` fixtures still decode; a `Request`
  `Hello` decoded by the previous `Intent` type fails cleanly; an advert
  without `rp` parses as `request_port: None`.

### 7. Amendment to ADR 0002

ADR 0002 made linking "pair-only, no moving". **Linking a computer now also
lets it request a move** (and lets this computer request one from it). The
receiver still reviews and approves every move, and it can turn requests
off per device. Phones are never candidates: linking a phone grants no move
requests in either direction.

## Alternatives considered

| Option | Why not |
|---|---|
| **B. Request over the open link connection** (a lane message) | No handshake change, but works only while both are linked and connected: known-only computers would still need a code, and moves would depend on link uptime and link policy. |
| **C. Plan's separate intent: `Request { from: Fingerprint, nonce, proof = HMAC(KnownSecret, binding ‖ nonce) }` + identity signature** | Duplicates the link machinery (tag lookup, proofs, nonce set) with new code. Sending `from` names the dialer to anyone on the LAN (ADR 0002 rejected this for links), and a fingerprint is display-only (ADR 0001 §2). |
| Reuse the link tag label for requests | A captured link `Hello` would pass a request listener's admission (separate nonce set). Own label costs one constant. |
| New proof labels / `SignContext::DeviceRequest` | Proofs are bound to the connection's binding and role; the intent already says what the connection is for. More code, no attack prevented. |
| Run the move on the request connection | Needs a resume secret derived from `KnownSecret` (new crypto) and a session whose sender is the dialer (today the move sender listens, `engine/send/mod.rs:36-104`). |
| Receiver dials back with a new `Intent` instead of a code | Same need for a resume secret, plus a second new intent. The code reuses SPAKE2, resume and `Identify` as they are. |
| Request on the link listener's port (`lp`) | Only runs with the link supervisor; known-only computers would not receive requests. |
| Unknown devices may request while visibility is Everyone (spec 2026-11-12, "Technical calls") | User decision: unknown computers always need a code. A stranger can never cause a notice. |
| New `RequestFailed` close code | Same meaning as `LinkFailed`; a distinct code tells an attacker nothing more and costs a wire value. |

## Consequences

- `carrova-crypto`: label `carrova/1/known-request`, `RequestTag`,
  `KnownSecret::{request_tag, recognises_request}`. No change to HKDF,
  proofs, or signature contexts.
- `carrova-transport`: `Intent::Request`, `Handshake::{Request, Review,
  Proceed}`, `RequestSummary`, `CloseCode::Declined`, `Expected::Request`
  / `Admitted::Request`, `Listener::accept_request` → `IncomingRequest`
  (`peer`, `summary`, `review() -> Proceed`, `decline()`), a dialer call
  that sends a request and waits for its end, `TXT_REQUEST_PORT` and
  `request_port`, per-minute admission budget, pending cap, pinned move
  pairing (both sides), `must_identify` with a pin.
- Core presence: tag order puts requests-on computers first while the
  request listener runs (Decision 3, tag limit); trusted computers beyond
  13 tags need a code.
- Supersedes plan Task 3.0/3.1: no `from: Fingerprint` in the `Hello`;
  the store is keyed by full public key (ADR 0001 Q8); `KnownLookup` is the
  existing `KnownPeers`; `admission.rs` exists and is extended, not
  created; `IncomingRequest::accept` yields a `Proceed`, not an
  `Authenticated` session.
- Known store: `DeviceRecord` gains `send_requests: bool`
  (`#[serde(default = true)]`, ignored for phones).
- Every requested move is a code-paired move under the hood, so it shows up
  in history, resume and remember exactly like one.

## Threat model

Additions to ADR 0001 and 0002.

| Threat | Mitigation | Residual risk |
|---|---|---|
| **Stranger triggers a notice** | No notice before both proofs and both signatures passed and `Request` arrived. Unknown, forgotten, phone and requests-off devices are not candidates and are closed `LinkFailed` without reply. | None known. |
| **Flooding the listener** | QUIC Retry, 5 attempts/min/IP, 4 concurrent handshakes and 1 per IP, ≤ 64 HMACs per `Hello`, 3 pending, 1 pending per device, 2 min lifetime. | A trusted device can keep one request pending at a time. A stranger with many IPs can keep handshake slots busy (attempts fail at the tag); a real request then retries. |
| **Replayed request `Hello`** | Nonce once per listener; proofs bound to the channel. | After a listener restart a replay reaches the proofs and fails. |
| **Link `Hello` replayed as a request (or the reverse)** | Distinct tag labels; distinct intents. | None known. |
| **Lying summary** (dialer claims "2 apps, no keys") | Counts are hints. Review runs today's dry run on the real manifest; sensitive items still default off and are asked one by one. | A user may decline or accept on false counts; nothing is written before the real plan is approved. |
| **Names leak on a locked screen** | The notice is built from the local record name, shown only per the lock-screen setting (design doc, open question 2). Counts are never on a locked screen. | Device name exposure depends on the setting chosen. |
| **Code in `Proceed` is intercepted** | Sent only inside the mutually authenticated TLS channel, only after Review; never shown, logged or advertised; single-use; the move sender's listener does not advertise its nameplate or name (only the resume tag, after pairing). | An attacker who finds the port and guesses nameplate and words in one try (as today's code moves) burns the code; the dialer then ends with "Send again". |
| **Wrong computer answers the move connection** | Receiver pins the requester's key; dialer pins the requested receiver's key. | None known. |
| **Stolen device on the LAN** | Same as links (ADR 0002): it can request until forgotten, but every move is still reviewed and approved on the receiver. | User may approve a request from a stolen laptop believing it is theirs. |

## Logging

ADR 0001 and 0002 lists apply. Additionally never logged: request tags and
nonces, `Proceed` (port is fine, code never), the summary together with a
device name at info level or above. A failed request logs its failure kind
only.

## Open questions for the user

1. **One-time code inside the request** (Decision 4) instead of running the
   move on the request connection. Recommended: yes (no new crypto; resume,
   remember and history unchanged).
2. **Reuse link proofs** (`carrova/1/link-confirm`, `DeviceLink`) on the
   request connection, with only a new tag label. Recommended: yes.
3. **Reuse `LinkFailed`** for failed requests and add only `Declined` = 7.
   Recommended: yes.
4. **One pending request per device**, newer replaces older
   (`Superseded`). Recommended: yes (stops one device filling all 3
   slots).
5. **Own listener and `rp` TXT key** rather than the link listener.
   Recommended: yes.
6. **Amendment to ADR 0002** (Decision 7) as worded. Recommended: yes.
