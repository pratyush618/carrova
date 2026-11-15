# ADR 0002: Linked devices (pair-only, QR pin, code-less link)

- Status: Proposed
- Date: 2026-11-15
- Scope: Ecosystem Phase 1, "Gate 0"
  (`docs/superpowers/specs/2026-11-15-ecosystem-phase1-design.md`), plan
  Tasks 1–4 (`docs/superpowers/plans/2026-11-15-ecosystem-phase1.md`).
- Builds on: ADR 0001 (device key, `KnownSecret`, `Identify`, known tags).
- Touches: `carrova-crypto` (one new tag purpose, link proofs, one new
  signature context) and the transport handshake (new `Hello` field, new
  intent, new frames, new close codes). Both need explicit sign-off
  (CLAUDE.md, "Crypto").

## Context

ADR 0001 lets two computers remember each other after a code-paired move.
Phase 1 needs more: a phone and a computer (or two computers) that pair
once **without moving anything**, then reconnect on their own whenever both
are on the LAN, with no code.

What exists (ADR 0001 implemented):

- `Hello { proto, intent, identify }`; `identify` is `#[serde(default)]`
  so older peers omit it (`crates/carrova-transport/src/wire/messages.rs:12-22`).
  Frames are a serde-derived enum without `deny_unknown_fields`, so an old
  peer ignores unknown map keys.
- `Intent` has `Pair { nameplate }` and `Resume { nonce: [u8; 16], tag:
  [u8; 16] }` (`wire/messages.rs:48-64`). `RejectReason` is `Version` or
  `NotOffered` (`wire/messages.rs:68-73`).
- `Identify { public, sig }` runs on code pairing after both `Confirm`s and
  before `Established` (`wire/messages.rs:34-41`,
  `handshake/mod.rs:3-14`, `handshake/sender.rs:177-187`,
  `handshake/identify.rs:22-50`). A bad `Identify` fails the pairing with
  `TransportError::PairingFailed`.
- The sender rejects any `Hello` whose `proto` differs from
  `PROTOCOL_VERSION = 1` (`handshake/sender.rs:98-99`,
  `crates/carrova-transport/src/lib.rs:74`). One PAKE guess per code
  (`handshake/sender.rs:114-119`).
- Close codes: `Normal`, `PairingFailed`, `Protocol`, `Rejected`,
  `Cancelled` = wire 0–4; unknown codes decode as `None`
  (`wire/close.rs:7-42`).
- What a listener accepts is `Expected::{Pair, Resume}`; a resume `Hello`
  is admitted only if its tag verifies **and** its nonce is new
  (`handshake/secret.rs:66-98`). Nonces live in `ResumeNonces`, a bounded
  FIFO set per listener (`crates/carrova-transport/src/replay.rs:12-46`),
  capacity `RESUME_NONCE_MEMORY = 1024` (`tuning.rs:64`,
  `listener.rs:76,108`).
- `KnownSecret` (32 bytes, zeroized, redacted `Debug`) makes discovery
  tags `HMAC(secret, label ‖ nonce ‖ advertiser DevicePublic)[..8]` and
  checks them with `verify_truncated_left` (constant time)
  (`crates/carrova-crypto/src/pairing/known.rs:20-82`). `KnownPurpose` has
  only `Discovery`, label `carrova/1/known-discovery`
  (`known.rs:94-105`, `pairing/mod.rs:119`). `KNOWN_TAG_LEN = 8`,
  `TAG_NONCE_LEN = 16` (`pairing/mod.rs:93-95`).
- `MacKey::tag_mac(label, nonce)` = `HMAC(key, label ‖ nonce)` and
  `MacKey::role_mac(role, binding)` = `HMAC(key, role ‖ binding)`
  (`pairing/keys.rs:33-46`). `ResumeSecret::prove/verify` build the resume
  proof from `role_mac` (`pairing/keys.rs:103-124`). `KnownSecret` has no
  `prove`/`verify` today.
- `DeviceKey::sign_binding` / `verify_peer` sign `role ‖ ChannelBinding ‖
  own public key` under `SignContext::DeviceIdentity`, reject a peer
  claiming our own key, and verify strictly
  (`crates/carrova-crypto/src/identity.rs:87-115`). `SignContext` tags end
  in NUL (`sign/mod.rs:53-72`); `KeyRole::permits` is the one place that
  says which roles may sign which context; `BundleSigner` permits only
  `BundleIndex` (`sign/cert.rs:21-31`).
- Presence adverts carry `kt` (nonce + up to 13 8-byte tags) and, when
  shown to everyone, `dn`/`os` (`discovery/presence.rs:18-36`,
  `lib.rs:88-96`). The beacon advertises `BROADCAST_PORT` as its port,
  marked `shortcut:` because no listener exists yet
  (`discovery/presence.rs:161-163`, `lib.rs:98`). `Nearby` exposes the
  advert and the address, not a port (`discovery/browse.rs:21-30`).
- `Ticket` text is `<code>` or `<code>@<ipv4>:<port>`
  (`crates/carrova-transport/src/ticket.rs:12-22`).
- Peer device names: at most `MAX_DEVICE_NAME = 64` characters
  (`crates/carrova-manifest/src/model.rs:56`), cleaned of control and bidi
  characters in core (`crates/carrova-core/src/lan/messages.rs:119-133`).
- `OsKind` lives in `carrova-platform` and transport already uses it
  (`crates/carrova-platform/src/os.rs:11-18`, `discovery/presence.rs:12`).
- `TransportError` has no key-mismatch or link variant today
  (`crates/carrova-transport/src/error.rs:13-58`).

No new dependency is needed (`hmac`, `sha2`, `ed25519-dalek`,
`data-encoding` are already used by `carrova-crypto`).

## Decision

Roles: as today, the side that sends `Hello` is the **receiver**
(`PakeRole::Receiver`) and the side that listens is the **sender**
(`PakeRole::Sender`). On a link, the dialer is the receiver and the
listener is the sender.

### 1. Pair-only

```text
R → S  Hello { proto: 1, intent: Pair { nameplate }, identify: true, purpose: PairOnly }
S → R  Welcome
       ... PAKE, sender Confirm, receiver Confirm (unchanged) ...
S → R  Identify(sender)          R → S  Identify(receiver)      (ADR 0001, unchanged)
S → R  Established
S → R  PeerInfo { name, os }     R → S  PeerInfo { name, os }
S → R  Linked
both   close Normal
```

- `Hello` gains `#[serde(default)] purpose: PairPurpose`, an enum
  `{ #[default] Move, PairOnly }`. Older receivers omit it and read as
  `Move`.
- New frames `Handshake::PeerInfo { name: String, os: OsKind }` and
  `Handshake::Linked`.
- A `PairOnly` `Hello` without `identify: true` is rejected
  (`Reject(NotOffered)`, code not burned). A listener offering a pair-only
  code rejects a `Move` `Hello` for that nameplate, and the reverse, the
  same way: the purpose is part of what is offered.
- Pair-only always identifies: it does not depend on the "Remember devices
  I move with" setting (that setting governs moves only). Both sides must
  hold a device key; without one the call fails locally before connecting.
- `PeerInfo` is exchanged after `Established` and before `Linked`, so each
  side records the peer's real name and OS. The link itself never re-sends
  them. The name is at most `MAX_DEVICE_NAME` (64) characters; a longer
  one is a protocol violation. It is cleaned before display and storage as
  today. It is not trusted for anything but display.
- Frames after `Established` are safe here (unlike ADR 0001's case): both
  sides know from `purpose` that no session follows.
- Storing: the sender stores the `KnownSecret` and the record (full public
  key, name, OS) before sending `Linked`; the receiver stores them on
  `Linked`. A side that fails before its store keeps nothing.

### 2. QR payload and key pin

- `carrova://pair?v=1&t=<ticket>&k=<base32 DevicePublic>`. `<ticket>` is
  the existing `Ticket` text (`code` or `code@ip:port`), percent-encoded as
  a query value. `k` is the sender's 32-byte public key, RFC 4648 base32
  without padding (52 chars).
- When `k` is present, the scanning side (receiver) checks the sender's
  `Identify` key right after it verifies. On mismatch it does not send its
  own `Identify`, closes with `PairingFailed`, and returns the new error
  `TransportError::KeyMismatch` (UI: "This isn't the computer you
  scanned"). The phone never reveals its key to an impostor.

### 3. Link intent and tag

- `Intent::Link { nonce: [u8; 16], tag: [u8; 16] }`.
- `KnownPurpose::Link`, label `b"carrova/1/known-link"`.
  `tag = HMAC-SHA256(KnownSecret, "carrova/1/known-link" ‖ nonce ‖ dialer's
  DevicePublic)[..16]`, fresh random nonce per attempt. Same construction
  as the discovery tag (`MacKey::tag_mac` + key), so `KnownSecret` gets
  `link_tag` / `recognises_link`.
- **16-byte tag (`LINK_TAG_LEN = 16`), not 8 like `KNOWN_TAG_LEN`.** The
  discovery tag is a public hint that only costs a failed connect when it
  falsely matches, and 13 must fit one TXT string. The link tag is the
  admission check of an online endpoint: every guess is one `Hello`, and
  the listener compares it against up to 64 secrets. At 64 bits that is a
  2^-58 chance per guess of passing admission (still stopped by the proofs
  below); at 128 bits it is 2^-122. It costs 8 bytes in one frame and
  matches `RESUME_TAG_LEN`. A new type `LinkTag([u8; 16])` with redacted
  `Debug` keeps it from being mixed up with a `KnownTag`.
- **Listener lookup.** The `Hello` carries no key. The listener tries each
  known `(secret, peer key)` pair with `recognises_link(nonce, peer key,
  tag)`, checks **all** candidates with no early exit, and keeps the
  match. Each check is constant time. At most **64 candidates**: beyond
  that, the 64 most recently used are tried and older ones cannot link
  until used again (by a code pairing or a move). The limit bounds the
  HMAC work an unauthenticated `Hello` can cause (64 HMACs).
- **Nonce admission.** A recognised `Link` `Hello` is admitted once per
  listener: its nonce goes through the same `ResumeNonces` set
  (`first_use`) as resume `Hello`s, capacity 1024. Unrecognised `Hello`s
  are not recorded (they cannot fill the set).
- Then `Welcome`, and both sides prove, in this order:

```text
R → S  Hello { proto: 1, intent: Link { nonce, tag } }
S → R  Welcome
S → R  Confirm(KnownSecret proof, sender)
R → S  Confirm(KnownSecret proof, receiver)     (only after the sender's verified)
S → R  Identify(sender, DeviceLink sig)
R → S  Identify(receiver, DeviceLink sig)       (only after the sender's verified)
S → R  Established
```

- `KnownSecret::prove(role, binding)` / `verify(peer_role, binding,
  proof)` = `HMAC(KnownSecret, "carrova/1/link-confirm" ‖ role identity ‖
  ChannelBinding)`, a `Confirmation`, constant-time compare. The label
  is added to the spec's `HMAC(KnownSecret, role ‖ ChannelBinding)`
  because the same secret also keys discovery and link tags; every input
  under one key starts with its own label (the plan, Task 1, already
  names it).
- Then Ed25519 under the new `SignContext::DeviceLink`, tag
  `b"carrova/device-link/v1\0"`, over `role identity ‖ ChannelBinding ‖
  own public key` (same payload as `DeviceIdentity`, different context, so
  a pairing `Identify` never verifies as a link proof or the reverse).
  `KeyRole::permits` returns false for `DeviceLink` for every role.
- The link reuses the `Confirm` and `Identify` frames. Each side requires
  the `Identify` key to equal the expected peer: the dialer's stored key
  for this peer; on the listener, the key of the candidate whose tag
  matched. A peer claiming our own key is refused (as `verify_peer`).
- Any failure (no matching tag, replayed nonce, bad proof, bad signature,
  wrong key) closes with the new `CloseCode::LinkFailed`. No detail is sent
  to the peer; the local log records the failure kind only. The dialer
  gets `TransportError::LinkFailed`.

### 4. One connection per pair

- The side with the lower `DevicePublic` (bytewise) dials; the other only
  listens.
- If a second link between the same two keys is established, the newest
  connection from the dialer wins and the older one is closed with the new
  `CloseCode::Superseded`.
- Close codes: `LinkFailed` = 5, `Superseded` = 6 (appended; older peers
  decode them as unknown).

### 5. Forget

- Forgetting a device deletes the record and the secret (ADR 0001 order:
  wipe secret first) and drops its live link at once (close `Normal`).
- A link attempt with no matching secret is rejected exactly like a bad
  tag (`LinkFailed`), so a forgotten peer learns nothing more.

### 6. Presence carries the link port

- New TXT key `lp` (`TXT_LINK_PORT`): the link listener's QUIC port in
  decimal, present only while a link listener runs. It replaces the
  `BROADCAST_PORT` shortcut in `discovery/presence.rs:161-163`.
- `PresenceAdvert` and `Nearby` gain `link_port: Option<u16>`; an advert
  without `lp` (older peer) parses as `None`.
- A TXT key and not the SRV/reply port because broadcast-browse replies
  and `Nearby` expose TXT entries but not a presence's port
  (`discovery/browse.rs:21-30`). `lp=65535` adds at most 9 bytes to a
  broadcast reply; the 512-byte budget in `presence.rs:18-21` must be
  re-checked when it is added.

## Alternatives considered

| Option | Why not |
|---|---|
| Reuse a move (code pairing) to link | Phones have nothing to move; a link must not need a manifest, dry-run or apply. |
| Send the dialer's public key in `Intent::Link` | Saves the candidate scan, but every link attempt would name the device to anyone on the LAN (a stable identifier). The tag binds the key without revealing it. |
| 8-byte link tag like discovery | 2^-58 per online guess across 64 candidates. Cheap to do better, and the frame has room. |
| Only the HMAC proof, no signature | The `KnownSecret` alone proves the pairing, but a stolen secret file (without the device key) would then be enough to impersonate. Requiring both matches ADR 0001: secret and key. |
| Only the signature, no HMAC proof | A device key alone would let a device that was forgotten (secret deleted on this side) re-link while its key is still stored elsewhere; the secret is what "forget" removes. |
| Bump `PROTOCOL_VERSION` | Old senders reject every `Hello` with another `proto` (`handshake/sender.rs:98-99`), breaking moves with older Carrova. New fields default; new intents are rejected by old peers' serde as an unknown variant, which fails only that connection. |
| Both sides dial | Two connections per pair, races on every reconnect. The bytewise rule needs no negotiation. |
| New `LinkConfirm` / `LinkIdentify` frames | Same shapes as `Confirm` / `Identify`; the intent already tells both sides which construction applies. |

## Consequences

- `carrova-crypto`: `LINK_TAG_LEN`, `LinkTag`, `KnownPurpose::Link`,
  `KnownSecret::{link_tag, recognises_link, prove, verify}`, label
  constants `carrova/1/known-link` and `carrova/1/link-confirm`,
  `SignContext::DeviceLink`, `DeviceKey::{sign_link, verify_link_peer}`.
- `carrova-transport`: `PairPurpose`, `Hello.purpose`, `Intent::Link`,
  `Handshake::{PeerInfo, Linked}`, `CloseCode::{LinkFailed, Superseded}`,
  `TransportError::{KeyMismatch, LinkFailed}`, a link listener that looks
  up candidates through a trait (`KnownPeers`), `ResumeNonces` reused for
  link nonces, `TXT_LINK_PORT`, `link_port` on `PresenceAdvert`/`Nearby`,
  and the `presence.rs` shortcut removed.
- `PROTOCOL_VERSION` stays 1.
- Linking makes the device known without a move. The device store (ADR
  0001 storage, unchanged) now also holds phones; `OsKind` gains
  `Android` (ADR 0003).
- A pair-only `Hello` sent to an **old** sender is read as a move pairing
  (old serde ignores `purpose`). The old sender then sends `Established`
  with no `Identify`, and the new receiver fails with "peer too old" before
  storing anything. Acceptable: only happens if a user types a move code
  into "Link a computer".

## Threat model

Additions to ADR 0001's table.

| Threat | Mitigation | Residual risk |
|---|---|---|
| **Stolen phone on the LAN** | It holds a valid secret and device key, so it can link until the user forgets it on the computer. Quiet features (clipboard, ring) work; ask features (files, handoff) still ask on the computer. | Clipboard flows to the thief until forgotten. The UI must make "Forget" easy from the computer. |
| **Replayed `Hello`** (attacker captures a `Link` `Hello` and resends it) | Nonce admitted once per listener (`ResumeNonces`), and even an admitted replay fails the proofs: they are bound to the new TLS channel. | A listener restart forgets nonces; a replay then reaches the proofs and fails there. |
| **Downgraded peer** (`Link` with `proto` other than 1) | The sender rejects any `proto` ≠ 1 before reading the intent (`handshake/sender.rs:98-99`). There is no `proto` < 1 that knows `Link`. | None known. |
| **Tag linkability** (outsider tracks a device by its link attempts) | Fresh random nonce per attempt; the tag is an HMAC under the shared secret; the `Hello` carries no key. | IP, MAC and timing still link attempts (as ADR 0001). |
| **Online tag guessing** | 16-byte tag; at most 64 candidates; failures close with no detail; a guess that passes admission still needs the secret and key to prove. | Each guess costs the listener up to 64 HMACs (bounded). |
| **Impostor at QR pairing** (someone else's computer answers the scanned code) | `k` pins the sender's key; mismatch fails before the phone sends its own `Identify` (`KeyMismatch`). | Typed codes (no QR) have no pin; same as ADR 0001's code-holding attacker. |
| **Stolen `KnownSecret` file without the device key** | The link also needs the `DeviceLink` signature. | A stolen full state dir (both) impersonates until forgotten (ADR 0001). |
| **Spoofed `PeerInfo` name/OS** | Display only; identity is the full public key; names are cleaned. | A peer can call itself anything at pair time; the UI shows the fingerprint beside it. |
| **Duplicate-connection churn** (attacker forces `Superseded`) | Only a peer that passes the link proofs can open a link; the rule only replaces a connection with one from the same verified key. | None beyond the stolen-device case. |

## Storage

Unchanged from ADR 0001 (`device.key`, `devices/known.json`,
`devices/<public key hex>.key`, same modes, locking and write rules). Link
policy files (`devices/policy/<hex>.json`) are specified in the Phase 1
spec, not here. Nothing about a link (nonces, tags, proofs) is persisted.

## Logging

ADR 0001's list applies. Additionally never logged: link tags, link
nonces, link proofs and signatures, the QR payload, the `k` value. A failed
link logs the failure kind only (no peer key, no candidate index).

## Open questions for the user

1. **`carrova/1/link-confirm` label** in the link HMAC proof (the spec
   writes `HMAC(KnownSecret, role ‖ ChannelBinding)` without a label).
   Agree to add it?
2. **64 candidates, most recently used first**: acceptable limit, with
   older devices unable to link until used again?
3. **Pair-only ignores the "Remember devices I move with" switch** (it
   always stores, since storing is its purpose). Agree?
4. **`lp` TXT key** versus advertising the link port as the
   presence's SRV/broadcast-reply port. The ADR picks `lp` (the brief's
   choice). Agree?
5. **Stolen phone**: is "Forget from the computer" enough for Phase 1, or
   do you want an explicit "Pause all links" on the computer too?

Signed off: pending.
