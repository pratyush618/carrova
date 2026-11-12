# ADR 0001: Known-device identity

- Status: Accepted
- Date: 2026-11-12
- Signed off: 2026-11-12 by the user, with the answers below
- Scope: Phase 2 of `docs/superpowers/plans/2026-11-12-background-nearby.md`
  (Tasks 2.1–2.3). Phase 3 (code-less requests, Task 3.0) builds on it and
  gets its own ADR.
- Touches: `carrova-crypto` (new identity and known-secret types, one new
  HKDF output, one new signature context) and the transport handshake (new
  `Identify` exchange). Both need explicit sign-off (CLAUDE.md, "Crypto").

## Context

Today every move needs a pairing code. Two computers share nothing after a
session ends, so they cannot recognise each other later.

What exists:

- Pairing runs SPAKE2 inside mutual TLS 1.3 (QUIC) with throwaway
  certificates (`crates/carrova-crypto/src/pairing/mod.rs:1-29`). The SPAKE2
  key goes through HKDF-SHA256 with salt `carrova/1/pake` and two infos,
  `carrova/1/confirm` and `carrova/1/resume`
  (`pairing/mod.rs:102-106`, `pairing/pake.rs:100-119`).
- `SessionKeys` keeps only the confirmation key and the resume secret, not
  the SPAKE2 key or the HKDF PRK (`pairing/pake.rs:131-134`). Any new
  per-pair secret must be expanded in `Pake::finish`, next to the others.
- `ChannelBinding` = TLS exporter (`EXPORTER-carrova/1/pair`, 32 bytes) ‖
  SHA-256(sender cert) ‖ SHA-256(receiver cert), 96 bytes, zeroized and
  redacted in `Debug` (`pairing/binding.rs:53-84`, `pairing/mod.rs:74-80`).
  Key confirmation is `HMAC(K_confirm, role ‖ binding)`
  (`pairing/keys.rs:32-38`).
- The resume secret is per session. It is stored as `resume.key`, mode
  `0600`, created with `O_EXCL|O_NOFOLLOW`, owner- and size-checked on read,
  and wiped (zeros, unlink, dir sync) when the session ends
  (`crates/carrova-core/src/store/resume_key.rs:16-122`).
- Resume tags: `HMAC(resume_secret, label ‖ nonce)[..16]` with a fresh
  16-byte nonce per advert and distinct labels per purpose
  (`carrova/1/resume-tag`, `carrova/1/resume-hello`;
  `pairing/keys.rs:70-95`, `pairing/keys.rs:149-168`). Discovery puts
  `<nonce hex><tag hex>` in TXT key `rt`
  (`crates/carrova-transport/src/lib.rs:77-80`,
  `discovery/seek.rs:15-55`). `Hello` nonces are admitted once per listener
  (`crates/carrova-transport/src/replay.rs:33`,
  `handshake/secret.rs:69-83`).
- Handshake order: `Hello` → `Welcome` → PAKE → sender `Confirm` →
  receiver `Confirm` → `Established` (`handshake/mod.rs:3-12`). Frames are
  CBOR (`ciborium`, serde derive; `wire/frame.rs:13-29`). A sender rejects
  any `Hello` whose `proto` differs from `PROTOCOL_VERSION = 1`
  (`handshake/sender.rs:90-101`, `lib.rs:70`).
- Ed25519 signing already exists for recipe bundles: `SigningKey`
  (zeroize on drop, redacted `Debug`, seed from `getrandom`),
  `VerifyingKey::verify` with `verify_strict`, and per-purpose
  domain-separation tags `SignContext::tag()` that end in NUL
  (`crates/carrova-crypto/src/sign/mod.rs:16-33`, `sign/mod.rs:52-76`,
  `sign/keys.rs:16-60`, `sign/keys.rs:93`).
- The mDNS instance name is `carrova-<first 12 hex of the cert hash>`, per
  listener (`crates/carrova-transport/src/listener.rs:79-87`).

Workspace crates available, all already used by `carrova-crypto`
(`Cargo.toml:29-97`, `crates/carrova-crypto/Cargo.toml`):
`ed25519-dalek` 3.0.0 (`fast`, `zeroize`, `alloc`), `hkdf` 0.12.4,
`hmac` 0.12.1, `sha2` 0.10.9, `spake2` 0.4.0, `getrandom` 0.4, `zeroize` 1.
No new dependency is needed.

## Decision

### 1. Device key

- Each install (per OS user, per state dir) has one Ed25519 key,
  `DeviceKey`, made on first need from 32 bytes of `getrandom`.
- `DeviceKey` wraps the existing `sign::SigningKey`; `DevicePublic` wraps
  `sign::VerifyingKey`. No new signing code.
- The identity of a device is its **full 32-byte public key**. Every lookup,
  store key, file name and trust decision uses the full key (or its full
  SHA-256). Never the fingerprint, never the name, never the MAC address.
- The key does not rotate on its own. It changes only if the user resets
  identity (see open question 4).

### 2. Fingerprint

- `Fingerprint` = first 4 bytes of SHA-256(public key), upper-case hex,
  split 4·4 with U+00B7: `7F3A·19C2`.
- It is a **display label only**. 32 bits give no security: an attacker can
  grind an Ed25519 key with a chosen 32-bit fingerprint in about 2^32 key
  generations (hours on one machine). It lets a user tell two of their own
  devices apart; it does not prove anything.
- Two known devices with the same fingerprint (about 1 in 2^32 per pair)
  must still work, because storage is keyed by the full key.

### 3. Identity exchange (code pairing only)

The exchange runs only on a code-paired connection (`Keys::Pake`), after
both key confirmations passed, and before `Established`. Resume connections
do not exchange identity.

```text
R → S  Hello { proto: 1, intent: Pair{..}, identify: true }   (new optional field)
S → R  Welcome
       ... PAKE, sender Confirm, receiver Confirm (unchanged) ...
S → R  Identify { public, sig }      (only if Hello.identify and sender remembers)
R → S  Identify { public, sig }      (only after the sender's verified)
S → R  Established
```

- Signed message, under a new `SignContext::DeviceIdentity` with tag
  `carrova/device-identity/v1\0`:

  ```text
  tag ‖ role identity (carrova/1/sender | carrova/1/receiver) ‖ ChannelBinding (96) ‖ own public key (32)
  ```

- The role prevents reflection (a peer echoing our own `Identify` back).
  The binding ties the signature to this TLS session, so it cannot be
  replayed into another connection. The public key in the message makes the
  signed bytes self-describing.
- The verifier uses `verify_strict`, rejects a peer public key equal to its
  own, and fails the session (close with `PairingFailed`) on any bad
  `Identify`. `KeyRole::permits` must return false for the new context, so a
  bundle signer can never sign device identity and the reverse.
- Proof of possession matters even inside the PAKE channel: without the
  signature, a peer that holds the code could claim another device's public
  key.

### 4. Known secret

- `KnownSecret` = HKDF-SHA256-Expand(PRK, info `carrova/1/known`, 32 bytes),
  where PRK is the existing HKDF-Extract of the SPAKE2 key with salt
  `carrova/1/pake`. It is a third output of `Pake::finish`, beside
  `carrova/1/confirm` and `carrova/1/resume`. The brief writes this as
  "HKDF(SessionKeys, …)"; `SessionKeys` cannot be the input because it does
  not keep the PRK (`pairing/pake.rs:131-134`).
- Domain separation: the three infos are distinct fixed strings under one
  PRK with a fixed output length, so the outputs are independent. The new
  label is a constant in `pairing/mod.rs` with the others.
- `KnownSecret` is kept only if both sides sent a valid `Identify`. It is
  stored against the peer's full public key.
- **Latest code pairing wins.** Each new code-paired session between the
  same two keys replaces the stored secret on both sides. "First session
  only" would leave no way to recover after one side forgets or loses its
  copy; a fresh code pairing re-syncs both. Resume connections never change
  it.
- It is never derived from, or stored with, the resume secret.

### 5. Discovery tag

- `KnownTag` = `HMAC-SHA256(KnownSecret, "carrova/1/known-discovery" ‖ nonce
  ‖ advertiser's 32-byte DevicePublic)`, truncated to 8 bytes, with a fresh
  random 16-byte nonce per advert. Otherwise the same construction as the
  resume tag (`pairing/keys.rs:70-95`). The brief's purpose string
  `known-discovery` gets the `carrova/1/` prefix used by every other label.
- Direction is bound (amended, signed off 2026-11-12): both peers hold the
  same `KnownSecret`, so without the advertiser's key a device would
  recognise its own advert as its peer's and list the peer as nearby while
  it is off. The advertiser tags with its own public key; a browser checks
  each tag against every known (secret, peer's stored public key) pair, so
  its own adverts never match.
- Purposes are an enum (`KnownPurpose`) like `TagPurpose`, so Phase 3 can
  add a request purpose with its own label. A discovery tag must never
  verify as a request proof.
- Checks are constant-time (`verify_truncated_left`, as `recognises` does
  today).
- A tag only says "a device that shares a secret with you is advertising
  here". It is not authentication. Phase 3 authenticates with both the
  `KnownSecret` and the device key over the new connection's binding.
- TXT layout (correction to the plan, see Consequences): one nonce per
  advert, shared by all tags in it, and the tags concatenated under one key
  `kt`. DNS-SD keys must be unique (RFC 6763 §6.4: only the first copy of a
  repeated key counts), and each TXT string is at most 255 bytes.

## Alternatives considered

| Option | Why not |
|---|---|
| MAC address as identity | Spoofable by anyone on the LAN. Modern Wi-Fi uses a random MAC per network. Not visible across routed subnets. It is personal data. Kept only as an opt-in Wake-on-LAN hint (Task 2.5). |
| Trust on first use without PAKE | The first contact would be open to a man in the middle, and any stranger could make itself "known". We already have an authenticated first contact (the code); reuse it. |
| Long-term X25519 key (static Diffie–Hellman, Noise-KK style) | Would avoid storing a per-pair secret, but needs a new key-agreement protocol design and either a Noise crate (not in the workspace) or our own composition, which the project rules forbid. Converting the Ed25519 key to X25519 mixes one key across two uses. |
| Reuse the resume secret | It is per session and wiped when the session ends (`store/resume_key.rs:101-122`). Reusing it would tie device trust to session lifetime and widen what a leaked `resume.key` gives. A separate HKDF info costs nothing. |
| `KnownSecret` only, no device key | Works for authentication, but each re-pairing would look like a new device (no stable identity to dedupe, rename or forget), and there is nothing to show as a fingerprint. |
| Device key only, no `KnownSecret` | Discovery tags need a shared symmetric secret to be unlinkable for outsiders. A public-key signature in an advert would identify the device to everyone. |
| Longer fingerprint (64 bits) | Not needed while the fingerprint is display-only. Revisit if users are ever asked to compare fingerprints to establish trust. |

## Consequences

- `carrova-crypto` gains `identity.rs` (`DeviceKey`, `DevicePublic`,
  `Fingerprint`), `pairing/known.rs` (`KnownSecret`, `KnownPurpose`), one
  HKDF info, one `SignContext` variant. `SessionKeys` gains
  `known_secret()`.
- The handshake gains an optional `Hello.identify` field and an `Identify`
  frame **before** `Established`. The plan (Task 2.2) says "after
  `Established`"; that would put a handshake frame where the session
  protocol expects its own frames, and an old peer would fail to decode it.
  Before `Established`, a new receiver reads either `Identify` (new sender)
  or `Established` (old sender, or remember off) and both are handshake
  frames it already knows how to read.
- `PROTOCOL_VERSION` stays 1. Bumping it would make old senders reject
  every `Hello` (`handshake/sender.rs:90-101`).
- Identity is exchanged only when both sides have "Remember devices I move
  with" on: the receiver sets `Hello.identify` only if it remembers; the
  sender sends `Identify` only if the flag is set and it remembers. No side
  stores a device that does not also store it.
- Anyone who completes a code pairing becomes known (if remember is on).
  That includes someone who saw the code over a shoulder and won the
  one-guess race. See open question 2.
- The plan's Task 2.1 interfaces key the store by `Fingerprint`
  (`forget(&Fingerprint)`, `secret_for(&Fingerprint)`, `<fp>.key`), and
  Task 3.0 sends `Request { from: Fingerprint }`. These must use the full
  public key (`DeviceId`, 32 bytes, hex in files). The fingerprint stays
  for display and for CLI input as a convenience that must resolve to
  exactly one known device or fail.
- TXT size caps the number of known tags per advert. With one shared nonce
  (32 hex) and 8-byte truncated tags (16 hex), one 255-byte string holds
  13 tags. A false match at 64 bits only costs a failed connect, because
  tags are hints. The plan's "one `kt` per known device, fresh nonce each"
  does not work as written. Cap: open question 5.
- The broadcast discovery fallback (UDP) must carry the same `kt` value.

## Threat model

| Threat | Mitigation | Residual risk |
|---|---|---|
| **Stranger on the LAN** (never paired) tries to be treated as known | Identity is only learned inside a PAKE-confirmed channel. A stranger cannot produce a known tag (needs `KnownSecret`) or an identity signature (needs the device key). Phase 3 rejects unknown keys before any UI. | Strangers can see that a Carrova device is at an IP: the mDNS instance, host label, IP and MAC are stable per listener. In Known mode they also see how many `kt` tags there are, i.e. roughly how many known devices (open question 6). |
| **Spoofed name** (stranger advertises `dn = "Alice's laptop"`) | Known status comes only from a matching `kt` tag and, on connect, the full public key. Names are never matched. Names of known devices come from the local store (set at pairing, renamable), not from adverts. Peer names are sanitised before display (`lib.rs:81-84`). | In "Everyone" mode a stranger can still show up as a *new* device with a familiar name. The UI must never show it as known. |
| **Fingerprint collision** (attacker grinds a key with a known device's 32-bit fingerprint) | Fingerprint is display-only; identity is the full key; storage and requests use the full key. | A user comparing fingerprints by eye could be fooled. Nothing in the protocol relies on that comparison. |
| **Stolen state dir** (attacker copies `device.key` and `devices/`) | Files are `0600`, owner-checked, created without following links, like `resume.key`. Same protection level as `~/.ssh`. | The thief can impersonate this device to every known peer and recognise their adverts until the user forgets them or resets identity. It cannot impersonate the peers to this device (it lacks their device keys). Disk encryption is the user's defence at rest. |
| **Replay of `Identify`** into another session | Signature covers the `ChannelBinding` (TLS exporter + both cert hashes) and the role. | None known. |
| **Replay of a discovery tag** (attacker rebroadcasts a captured advert) | Tags are hints only; any connection must prove `KnownSecret` and the device key over a fresh binding (Phase 3). | A known peer may show a device as "nearby" when it is not. Connect then fails. Adding a coarse time to the tag would narrow this but needs clocks to agree (open question 7). |
| **Linking adverts over time** by tag | Fresh nonce per advert; HMAC output is unlinkable without the secret. | Linking by IP, MAC and instance name remains (see first row). |
| **Code-holding attacker** at first pairing | One online guess per code (SPAKE2, single-use claim, `handshake/sender.rs:105-115`). | If it wins, it becomes a known device. Mitigation is UI (show "Remembered <name> (7F3A·19C2)" and offer Forget). |
| **One side forgets** | The forgetting side deletes the secret and record; it no longer recognises tags; Phase 3 closes unknown requests without reply. | The other side still lists the device and advertises its tag until the user forgets it there too. A send attempt fails and falls back to "Send with code", which re-pairs and replaces the secret (latest wins). Open question 3. |

## Storage and permissions

All under the state dir, through `carrova-platform` (no OS calls elsewhere):

| File | Content | Mode |
|---|---|---|
| `device.key` | 32-byte Ed25519 seed | `0600` |
| `devices/known.json` | records: full public key, name, OS kind, first seen, last move, move count, optional MAC hint | `0600` (it is the user's device graph) |
| `devices/<public key hex>.key` | 32-byte `KnownSecret` | `0600` |

- Write: `create_new_file` with mode `0600` (`O_EXCL|O_NOFOLLOW`), write,
  `fsync`, dir `fsync`, as `write_resume_secret`
  (`store/resume_key.rs:23-48`). Replacing a secret (re-pairing) writes
  `<id>.key.new` the same way, then renames over the old file and syncs the
  dir.
- Read: open without following links, `fstat` the open file, require a
  regular file, mode `0600`, owner = current user, exact size; same
  `PermissionModel::Acl` exception as today
  (`store/resume_key.rs:59-99`).
- A bad or corrupt `device.key` or `known.json` is reported, never silently
  regenerated or overwritten. Regenerating the device key would silently
  break every known relationship.
- Forget: wipe the key file first (zeros, sync, unlink, dir sync, as
  `wipe_resume_secret`, `store/resume_key.rs:101-122`), then remove the
  record. A crash between the two leaves a record without a secret, which
  is harmless and reported, never a secret without a record.
- In memory: `DeviceKey` and `KnownSecret` zeroize on drop and print
  `<redacted>` in `Debug`, like `ResumeSecret` (`pairing/keys.rs:122-126`).

### How the implementation handles the PR #8 review points

Three CodeRabbit points on PR #8 were left to this phase:

- **Interrupted secret replace.** `store::secret_file::replace` writes
  `<name>.new` with `create_new_file` (mode `0600`, `fsync`), renames it
  over the old file and syncs the dir. The old file is opened before the
  rename and zeroed through that handle after it. A crash leaves either the
  old secret or the new one in place, never a partial or zeroed file. The
  device key (`device.key`) is written the same way.
- **Stale `.key.new`.** A `.new` file left by a crash is wiped (zeros,
  sync, unlink) before the next replace writes a fresh one, and a failed
  rename wipes the `.new` it just wrote.
- **Overlapping pairings.** Every change to the store (remember, record a
  move, rename, forget, forget all, reset identity, first device key) runs
  under an exclusive lock on `devices/known.lock`, and `known.json` is
  replaced atomically, so reads take no lock. Two pairings with the same
  peer that finish together are applied one after the other on each host,
  so no update is lost. The lock is local: if the two hosts finish the
  pairings in opposite orders, they can keep different `KnownSecret`s and
  stop recognising each other. This is rare, and the next code pairing
  fixes it.

## Versioning and compatibility

- `Hello` gains `identify: bool` with `#[serde(default)]`. Old senders
  ignore the unknown CBOR map key (serde derive without
  `deny_unknown_fields`); old receivers omit it, so it reads as `false`.
  This must be proved by a test with a byte-for-byte old `Hello` fixture and
  a new `Hello` decoded by the old type, not assumed.
- New sender, old receiver: no flag, no `Identify`, pairing as today.
- Old sender, new receiver: flag ignored, receiver reads `Established`,
  `peer_identity = None`, pairing as today.
- New and new: exchange as in Decision 3; a bad `Identify` fails the
  session.
- Identity exchange fits inside the existing 10 s `HANDSHAKE_TIMEOUT`
  (`crates/carrova-transport/src/tuning.rs:51`).
- `known.json` carries a `version` field; unknown fields are kept or
  ignored, never cause a reset.

## Logging

Never logged, traced, put in errors, events, `Debug` output or error
reports:

- `DeviceKey` seed, `KnownSecret`, the SPAKE2 key, PRK, confirmation key,
  resume secret, `ChannelBinding` bytes.
- `Identify` signatures, discovery tags and nonces (they would let log
  readers link adverts).
- Full public keys (a stable cross-network identifier).
- MAC addresses.
- Pairing codes (unchanged rule).

Allowed: the display fingerprint (`7F3A·19C2`) and the user-chosen device
name in local logs at debug level. Error reports (`carrova-report`) redact
both. A failed `Identify` logs the failure kind only.

## Answers (signed off 2026-11-12)

All eight as proposed: (1) yes; (2) remember silently, show
"Remembered <name> (7F3A·19C2)" with Forget; (3) leave the other side
stale, no "forget me" notice now; (4) add "Reset this computer's
identity"; (5) cap 13 most recently used, 8-byte tags; (6) no padding;
(7) no time window; (8) full public key as store and request key.
Wake-on-LAN hints (Task 2.5) are in Phase 2.

## Open questions for the user (as asked)

1. **Identify before `Established`** instead of after, with a `Hello.identify`
   flag and no protocol version bump. Agree?
2. **Silent remember.** Remember right after a code-paired move (default on),
   or ask "Remember this computer?" on both sides first?
3. **One-sided forget.** Leave the other side's entry until its user
   forgets it (stale until a send fails and re-pairs), or add a signed
   "forget me" notice in a later phase?
4. **Reset identity.** Add "Reset this computer's identity" (new device key,
   forget all), for use after a stolen disk or a cloned state dir?
5. **Tag cap.** Up to how many known devices get a `kt` tag per advert
   (e.g. 13, most recently used), and 8-byte tags in TXT: acceptable?
6. **Hide the count.** Pad `kt` to a fixed number of tags with random filler
   so strangers cannot count known devices? Costs TXT space.
7. **Tag freshness.** Mix a coarse time (e.g. 10-minute window) into the
   discovery tag to limit replayed "nearby" ghosts? Needs both clocks within
   the window.
8. **Store key.** Confirm full public key as the store key and in Phase 3
   `Request`, with the fingerprint for display and CLI input only.
