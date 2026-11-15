# Ecosystem Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Linked devices (Linux laptop ↔ Android phone, Linux ↔ Linux) that pair once, stay connected on the LAN, and share clipboard, files, Handoff and Ring, from the desktop app, the Android app and the CLI.

**Architecture:** ADR 0002 adds pair-only and code-less `Link` handshakes to `carrova-transport`/`carrova-crypto`; transport gains public bidirectional streams. A new service crate `carrova-link` defines lanes and feature protocols. `carrova-core::ecosystem` owns policy, the link supervisor and a local control socket; `carrova-view` exposes view-models; CLI, Tauri desktop and the Tauri Android build are thin front-ends. Android OS access goes through `carrova-platform` (ADR 0003).

**Tech Stack:** Rust 1.99 / edition 2024, quinn 0.11 + tokio (transport only), ciborium, ed25519-dalek, hmac/sha2, BLAKE3 (already in manifest), ts-rs 12, clap 4, Tauri 2.12 (desktop + Android), React + i18next + Vitest/Playwright, Kotlin (Android glue only).

**Spec:** `docs/superpowers/specs/2026-11-15-ecosystem-phase1-design.md` (Phase 1, pinned values) and its parent `docs/superpowers/specs/2026-11-15-ecosystem-design.md`. Mockups: `docs/mockups/gui-ecosystem-desktop.html`, `docs/mockups/gui-ecosystem-android.html`. Read all four before starting.

## Before you start (new session)

1. Read `CLAUDE.md`, `tasks/lessons.md`, the two specs, ADR 0001 (`docs/adr/0001-known-devices.md`), then this plan.
2. Branch: `git switch -c feat/ecosystem-phase1` from `main`. Commit per task (Conventional Commits, ≤ ~60-char subject, **no AI attribution**, split unrelated changes). Push/PR only when the user asks.
3. Work in waves of ≤ 3 parallel subagents (laptop: 12 cores / 13 GiB). `cargo clean` between waves if `target/` > ~20 GiB, never mid-build.
4. Every brief to a subagent includes: senior idiomatic Rust, small modules, test-first, smallest complete change, tests outside `src/`, no logging of contents/keys/codes.
5. Run `boundary-reviewer` (`.claude/agents/`) after each part.

## Global Constraints

- OS-specific code only in `carrova-platform`; no `cfg(target_os)`, `/proc`, XDG, `std::os::unix` elsewhere (CLAUDE.md boundary rule 1).
- `src-tauri` holds no business logic; CLI and UI behave identically (rule 2). Dependencies point down: front-ends → core → services → platform; services never depend on core (rule 4).
- **Any change to `carrova-crypto` or the transport handshake needs the user's explicit sign-off via ADR before code.** Tasks 1–5 are blocked until Task 0 is signed off.
- `unsafe_code = "forbid"` everywhere except what ADR 0003 allow-lists (expected: one Android JNI module in `carrova-platform`).
- Libraries: `thiserror`, no `unwrap`/`expect`/`panic!` outside tests. CLI: `anyhow`. Logging via `tracing`; never log clipboard/file contents, file names, URLs, keys, secrets, pairing codes, device keys, MAC addresses.
- Tests live in `tests/<module>/` (`main.rs` integration, `unit.rs` via `#[cfg(test)] #[path = "…/tests/<module>/unit.rs"] mod tests;`); `just test-layout` enforces it. Bug fix → regression test first.
- New IPC types: `#[cfg_attr(feature = "ts", derive(ts_rs::TS))]`, added to the `exports` list in `crates/carrova-view/tests/export_bindings/main.rs`; run `just bindings`; `just bindings-check` must pass.
- Pinned values (from the Phase 1 spec): keepalive desktop 15 s / Android 45 s; idle timeout 3× keepalive; backoff 1 s doubling to 60 s, reset on network change; peer-gone close after 60 s; ask expiry 60 s; clipboard text ≤ 1 MiB, PNG ≤ 10 MiB; clipboard history 20; file chunk 1 MiB with BLAKE3; Ring 30 s on alarm stream; QR `carrova://pair?v=1&t=<ticket>&k=<base32 DevicePublic>`; policy file `<state>/devices/policy/<hex>.json` mode `0600` `{ "version": 1, "features": {…} }`; control socket `<state>/link.sock` mode `0600`; Android `minSdk` 29; app id `org.byteveda.Carrova`.
- Names: core must not reuse `Link` (taken by `crates/carrova-core/src/lan/link.rs:24`). Core types: `Supervisor`, `LinkedPeer`, `PolicyStore`. The crate is `carrova-link`.
- Done = `just ci` green with output shown.

## Review Focus

1. **Peer flaps** (Wi-Fi roam, suspend/resume while a file is mid-transfer): link must reconnect without duplicate connections and the transfer must resume or fail with "Device left", never write a half file under the final name. → tests in Task 15 (`duplicate_dial_keeps_one_connection`) and Task 9 (`interrupted_transfer_leaves_no_final_file`).
2. **Forget while linked** on one side only: the live connection drops at once, the other side's next `Link` attempt is rejected, and its UI shows "away", not an error loop. → Task 4 (`forgotten_peer_link_rejected`), Task 15 (`forget_drops_live_link`).
3. **Policy disagreement** (on here, off there): nothing flows; both UIs show the remote state; flipping one side takes effect without reconnect. → Task 7 (`gate_requires_both_sides`), Task 15 (`policy_change_applies_live`).
4. **Hostile file names and URLs** (`../x`, `con`, NUL, 300-char names, `file:`/`javascript:` URLs): normalised or refused before any write or open. → Task 9 (`safe_file_name_cases`), Task 10 (`handoff_rejects_non_http`).
5. **Second instance** (app running + `carrova link serve`, or two CLIs): exactly one supervisor; the other becomes a client of `link.sock`. → Task 16 (`second_supervisor_becomes_client`).

---

## Part A — Gate

### Task 0: ADR 0002 (linked devices) and ADR 0003 (Android bridge) — user sign-off

**Files:**
- Create: `docs/adr/0002-linked-devices.md`, `docs/adr/0003-android-platform-bridge.md`

- [ ] **Step 1: Write ADR 0002** in the shape of ADR 0001 (Context with file:line refs, Decision, Alternatives, Consequences, Threat model, Storage). Decision = the six points of "Gate 0" in the Phase 1 spec, concretely:
  - `Handshake::Hello` gains `#[serde(default)] purpose: PairPurpose` (`Move` default, `PairOnly`); new `Handshake::Linked`; `Intent::Link { nonce: [u8;16], tag: [u8;16] }`.
  - `KnownPurpose::Link` label `b"carrova/1/known-link"`, tag = `HMAC(KnownSecret, label ‖ nonce ‖ dialer DevicePublic)[..16]` — note this is a 16-byte tag (`LINK_TAG_LEN = 16`) unlike the 8-byte discovery tag; justify (online guess resistance per attempt).
  - Proofs after `Welcome` on a link: `KnownSecret::prove(role, binding)` (HMAC, `Confirmation`) both ways, then `Identify`-style Ed25519 under new `SignContext::DeviceLink` = `b"carrova/device-link/v1\0"`; `KeyRole::permits` false for it.
  - Nonce admission via `ResumeNonces`; listener lookup over all known (secret, key) pairs in constant time per candidate; max 64 candidates (beyond: most recently used first, document limit).
  - Close codes `LinkFailed`, `Superseded`; error `TransportError::KeyMismatch` for the QR pin.
  - Pair-only also exchanges `Handshake::PeerInfo { name: String (≤ MAX_DEVICE_NAME), os: OsKind }` both ways after `Established` and before `Linked`, so each side records the peer's real name and OS (the link itself never re-sends them).
  - Presence advert carries the link listener port (TXT `lp`), replacing the `BROADCAST_PORT` shortcut in `discovery/presence.rs`.
- [ ] **Step 2: Write ADR 0003** (Android bridge). Decide and record:
  - `OsKind::Android`; `carrova-platform/src/android/` selected by `#[cfg(target_os = "android")]`; `current()` gains the arm.
  - Rust→Kotlin calls via the `jni` crate using the VM/context from `ndk-context` (already in the lock file via tauri). This needs `unsafe` for `JavaVM::from_raw`; allow-list exactly `carrova-platform/src/android/jni.rs`: workspace lint stays `forbid`; `carrova-platform` sets `unsafe_code = "deny"` in its own `[lints]` and the one module has `#![allow(unsafe_code)]` with a `// SAFETY:` comment per block.
  - Kotlin glue classes live in `apps/desktop/src-tauri/gen/android/app/src/main/java/org/byteveda/carrova/platform/` and contain no decisions (one static method per platform call).
  - Process lifetime: Kotlin `LinkService` (foreground, `connectedDevice`) keeps the process alive; the Rust supervisor is started from the Tauri `setup` and, on boot, by `LinkService` starting the Tauri activity-less entry (record the mechanism the spike in Task 13 proves; if Tauri cannot start Rust without the activity, the fallback is `LinkService` calling `System.loadLibrary` + one exported `start_supervisor` through Tauri's mobile entry — record which).
  - QR scanning via the official `tauri-plugin-barcode-scanner`.
  - One sanctioned exception to boundary rule 1: `#[cfg_attr(mobile, tauri::mobile_entry_point)]` on `run()` in `apps/desktop/src-tauri/src/lib.rs` (Tauri's own mobile attribute; no other platform condition in `src-tauri`).
- [ ] **Step 3: Commit** `docs(adr): propose linked devices and Android bridge`.
- [ ] **Step 4: STOP. Ask the user to sign off ADR 0002 and ADR 0003** (show the Decision sections). Record "Signed off: <date> by the user" in each ADR and commit `docs(adr): record sign-off for ADR 0002 and 0003`. Do not start Tasks 1–5 or 12–13 before this.

---

## Part B — Crypto and transport (after sign-off)

### Task 1: Crypto primitives for linking

**Files:**
- Modify: `crates/carrova-crypto/src/pairing/mod.rs` (labels, `LINK_TAG_LEN`, re-exports), `pairing/known.rs` (`KnownPurpose::Link`, link tag, prove/verify), `identity.rs` (link sign/verify), `sign/mod.rs` (`SignContext::DeviceLink`), `sign/cert.rs` (`permits`)
- Test: `crates/carrova-crypto/tests/pairing/unit.rs`, `crates/carrova-crypto/tests/identity/main.rs` (or existing identity test file), `crates/carrova-crypto/tests/sign/…`

**Interfaces:**
- Produces:
  - `pub const LINK_TAG_LEN: usize = 16;` `pub struct LinkTag(pub [u8; LINK_TAG_LEN]);` (redacted `Debug`)
  - `KnownSecret::link_tag(&self, nonce: &TagNonce, dialer: &DevicePublic) -> Result<LinkTag>`
  - `KnownSecret::recognises_link(&self, nonce: &TagNonce, dialer: &DevicePublic, tag: &LinkTag) -> bool` (constant time)
  - `KnownSecret::prove(&self, role: PakeRole, binding: &ChannelBinding) -> Result<Confirmation>` and `KnownSecret::verify(&self, peer_role: PakeRole, binding: &ChannelBinding, proof: &Confirmation) -> Result<()>` (same construction as `ResumeSecret::prove/verify`, `pairing/keys.rs:103-114`, label-separated by a new `b"carrova/1/link-confirm"`)
  - `DeviceKey::sign_link(&self, role: PakeRole, binding: &ChannelBinding) -> [u8; IDENTITY_SIG_LEN]` and `DeviceKey::verify_link_peer(&self, peer: &DevicePublic, peer_role: PakeRole, binding: &ChannelBinding, sig: &[u8; IDENTITY_SIG_LEN]) -> Result<(), IdentityError>` under `SignContext::DeviceLink`

- [ ] **Step 1: Write failing tests**: `link_tag_roundtrip` (recognises own tag), `link_tag_bound_to_dialer` (other key → false), `link_tag_not_discovery_tag` (a `KnownPurpose::Discovery` tag bytes never verify as link), `link_proof_roundtrip`, `link_proof_rejects_reflection` (own role proof fails as peer), `link_proof_bound_to_binding`, `link_signature_not_identity_signature` (sig from `sign_binding` fails `verify_link_peer` and vice versa), `bundle_signer_cannot_sign_device_link` (`KeyRole::BundleSigner.permits(SignContext::DeviceLink) == false`), `device_link_tag_value` (`SignContext::DeviceLink.tag() == b"carrova/device-link/v1\0"`).
- [ ] **Step 2:** `cargo test -p carrova-crypto` → new tests FAIL to compile.
- [ ] **Step 3:** Implement the interfaces above (reuse `MacKey::tag_mac`/`role_mac`; no new dependency).
- [ ] **Step 4:** `cargo test -p carrova-crypto` → PASS; `cargo clippy -p carrova-crypto -- -D warnings` clean.
- [ ] **Step 5: Commit** `feat(crypto): add link tags, proofs and signatures`.

### Task 2: Wire messages for pair-only and link

**Files:**
- Modify: `crates/carrova-transport/src/wire/messages.rs`, `wire/close.rs`, `error.rs`
- Test: `crates/carrova-transport/tests/wire/unit.rs` (create if missing, wired from `wire/mod.rs`)

**Interfaces:**
- Produces: `pub(crate) enum PairPurpose { #[default] Move, PairOnly }`; `Handshake::Hello { proto, intent, identify, #[serde(default)] purpose: PairPurpose }`; `Handshake::Linked`; `Handshake::PeerInfo { name: String, os: OsKind }`; `Intent::Link { nonce: [u8; 16], tag: [u8; 16] }`; `CloseCode::LinkFailed`, `CloseCode::Superseded`; `TransportError::KeyMismatch`, `TransportError::LinkFailed`.

- [ ] **Step 1: Failing tests**: `old_hello_decodes_as_move` (CBOR of a Hello without `purpose` decodes with `PairPurpose::Move`), `link_intent_roundtrip`, `linked_roundtrip`, `close_codes_distinct` (all `CloseCode` values unique as varints).
- [ ] **Step 2:** run `cargo test -p carrova-transport wire` → FAIL.
- [ ] **Step 3:** add the variants/fields.
- [ ] **Step 4:** tests PASS; whole crate still compiles and existing handshake tests pass.
- [ ] **Step 5: Commit** `feat(transport): add pair-only and link wire messages`.

### Task 3: Pair-only handshake and QR pin

**Files:**
- Modify: `crates/carrova-transport/src/handshake/{secret.rs,receiver.rs,sender.rs,identify.rs}`, `connect.rs`, `listener.rs`, `lib.rs`
- Test: `crates/carrova-transport/tests/pair_only/main.rs` (uses `tests/support/mod.rs` helpers `direct_listener`, `ticket`, `T`)

**Interfaces:**
- Consumes: Task 2 messages.
- Produces:
  - `pub struct PairedPeer { pub identity: PeerIdentity, pub name: String, pub os: OsKind }`
  - `pub fn pair_only(ticket: &Ticket, pin: Option<&DevicePublic>, me: &PeerInfo, timeout: Duration, opts: &ConnectOptions) -> Result<PairedPeer>` (requires `opts.device`; `None` → `TransportError::Internal`); `pub struct PeerInfo { pub name: String, pub os: OsKind }`
  - `Listener::accept_pair_only(&mut self, code: &PairingCode, me: &PeerInfo, timeout: Duration, on_pake: impl FnMut()) -> Result<PairedPeer>` (requires the listener's device key)
  - Flow: Hello{identify: true, purpose: PairOnly} … Identify ×2 → `Established` → `PeerInfo` ×2 (names cleaned, over-long → `Violation`) → sender sends `Linked` → both close `Normal`.

- [ ] **Step 1: Failing tests**: `pair_only_returns_both_identities` (each side's `identity.public` = the other's key; secrets equal by `expose_bytes`; `name`/`os` = the other's `PeerInfo`), `pair_only_with_matching_pin_succeeds`, `pair_only_with_wrong_pin_fails_key_mismatch` (`Err(TransportError::KeyMismatch)`, and the listener side ends with `PairingFailed`), `pair_only_without_identify_rejected`, `move_pairing_unchanged` (existing `accept_pairing` + `Target::Pairing` still produce a `Connection`).
- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3:** implement; extend `Secret::Pair`/`Expected::Pair` with `purpose: PairPurpose` and `pin: Option<DevicePublic>`; the pin is checked right after the peer's `Identify` verifies.
- [ ] **Step 4:** `cargo test -p carrova-transport` → PASS.
- [ ] **Step 5: Commit** `feat(transport): add pair-only handshake with key pin`.

### Task 4: Link handshake, listener and presence port

**Files:**
- Modify: `crates/carrova-transport/src/handshake/*`, `connect.rs`, `listener.rs`, `accept.rs`, `replay.rs` (reuse), `discovery/presence.rs` (TXT `lp`), `discovery/browse.rs` (`Nearby.link_port: Option<u16>`), `lib.rs` (`TXT_LINK_PORT = "lp"`)
- Test: `crates/carrova-transport/tests/link/main.rs`, `tests/discovery/presence/unit.rs`

**Interfaces:**
- Produces:
  - `pub trait KnownPeers: Send + Sync { fn candidates(&self) -> Vec<(DevicePublic, KnownSecret)>; }`
  - `Target::Link { peer: &'a DevicePublic, secret: &'a KnownSecret, addr: SocketAddrV4 }` (requires `opts.device`)
  - `Listener::accept_link(&mut self, known: &dyn KnownPeers, timeout: Duration) -> Result<Connection>`
  - `Connection::peer_identity()` returns `Some` for link connections (public = verified peer key).
  - `PresenceAdvert { …, link_port: Option<u16> }`, `Nearby { …, link_port: Option<u16> }`.

- [ ] **Step 1: Failing tests**: `link_after_pair_only_connects` (pair-only, then `connect(Target::Link{…})` vs `accept_link` → both `peer_identity().public` correct), `link_bad_tag_rejected` (`Err(LinkFailed)` on dialer; listener keeps accepting), `link_wrong_device_key_rejected` (right secret, other key → fail), `forgotten_peer_link_rejected` (candidates empty → fail), `link_nonce_replay_rejected` (same Hello bytes twice → second fails), `link_from_old_proto_rejected`, `presence_advert_carries_link_port` (TXT roundtrip `lp`), `presence_without_lp_parses` (older advert → `None`).
- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3:** implement: dialer sends `Hello{intent: Link{nonce, tag}}`; listener scans candidates with `recognises_link` (all candidates checked, no early exit), admits nonce, `Welcome`, then prove/verify (`KnownSecret`) and sign/verify (`DeviceLink`) both ways, `Established`. Remove the `shortcut:` comment in `presence.rs` once `lp` is advertised.
- [ ] **Step 4:** `cargo test -p carrova-transport` → PASS.
- [ ] **Step 5: Commit** `feat(transport): add code-less link handshake`.

### Task 5: Public bidirectional streams on a connection

**Files:**
- Modify: `crates/carrova-transport/src/connection.rs`, `wire/messages.rs` (`StreamOpen::Lane`), `lib.rs`
- Create: `crates/carrova-transport/src/stream.rs`
- Test: `crates/carrova-transport/tests/stream/main.rs`

**Interfaces:**
- Produces:
  - `pub struct Stream` (blocking, runs on the crate runtime) with `send<T: Serialize>(&mut self, &T) -> Result<()>`, `recv<T: DeserializeOwned>(&mut self, Duration) -> Result<T>`, `write_all(&mut self, &[u8]) -> Result<()>`, `read_exact(&mut self, &mut [u8], Duration) -> Result<()>`, `finish(self) -> Result<()>`.
  - `Connection::open_stream(&self) -> Result<Stream>` (either side), `Connection::accept_stream(&self, Duration) -> Result<Stream>`; streams are tagged `StreamOpen::Lane` so chunk streams of move sessions never reach `accept_stream`.
  - `Connection::set_keepalive(&self, interval: Duration)` and idle timeout = 3× interval.
  - `Connection` becomes `Sync` for these methods (internal `Arc`), so the supervisor can share it.

- [ ] **Step 1: Failing tests**: `both_sides_open_streams`, `frames_and_raw_bytes_on_one_stream`, `frame_cap_enforced` (`FrameTooLarge`), `accept_stream_times_out`, `chunk_streams_not_seen_as_lanes`, `keepalive_keeps_idle_link` (interval 200 ms, idle 2 s, still connected).
- [ ] **Step 2:** run → FAIL. **Step 3:** implement. **Step 4:** PASS.
- [ ] **Step 5: Commit** `feat(transport): expose lane streams on connections`.

---

## Part C — `carrova-link` (protocols; no core dependency)

### Task 6: Crate, lanes and Ping

**Files:**
- Create: `crates/carrova-link/{Cargo.toml,src/lib.rs,src/lane.rs,src/error.rs,src/ping.rs}`; add to workspace deps.
- Test: `crates/carrova-link/tests/lane/main.rs`, `tests/support/mod.rs` (two linked `Connection`s over loopback via Tasks 3–4)

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)] pub enum LaneKind { Policy, Clipboard, File, Handoff, Ring, Ping }`
  - `pub struct LaneOpen { pub kind: LaneKind, pub version: u16 }` (wire, version 1); `pub enum LaneReply { Accepted, Refused(RefuseReason) }`, `pub enum RefuseReason { Unknown, NotAllowed, Busy }`
  - `pub fn open_lane(conn: &Connection, kind: LaneKind) -> Result<Stream, LinkError>` (sends `LaneOpen`, waits `LaneReply`)
  - `pub trait LaneGate { fn admit(&self, kind: LaneKind) -> Result<(), RefuseReason>; }`
  - `pub fn accept_lane(conn: &Connection, gate: &dyn LaneGate, timeout: Duration) -> Result<(LaneKind, Stream), LinkError>` (unknown kind/version → `Refused(Unknown)` sent, returns `LinkError::Refused` without dropping the connection)
  - `pub fn ping(conn: &Connection) -> Result<Duration, LinkError>` / `pub fn serve_ping(stream: Stream) -> Result<(), LinkError>`
  - `pub enum LinkError` (thiserror) wrapping `TransportError`, `Refused(RefuseReason)`, `Protocol(&'static str)`, `TooBig`, `Expired`, `Declined`.

- [ ] **Step 1: Failing tests**: `ping_roundtrip`, `unknown_lane_refused_connection_survives` (raw `LaneOpen{version: 99}` → refused; then ping works), `gate_refusal_reaches_opener` (`Err(LinkError::Refused(NotAllowed))`).
- [ ] **Step 2–4:** FAIL → implement → PASS (`cargo test -p carrova-link`).
- [ ] **Step 5: Commit** `feat(link): add lane protocol and ping`.

### Task 7: Features, policy documents and two-sided gate

**Files:**
- Create: `crates/carrova-link/src/policy.rs`
- Test: `crates/carrova-link/tests/policy/main.rs`

**Interfaces:**
- Produces:
  - `pub enum Feature { Clipboard, Files, Handoff, Ring, Notifications, Messages, Calls, Screen, Input }` (serde snake_case; last five reserved)
  - `pub enum FeatureState { On, Off, Ask }`; `pub enum Group { Quiet, Ask }`; `Feature::group(self) -> Group`; `Feature::PHASE1: [Feature; 4]`
  - `pub struct PolicyDoc { pub version: u32, pub features: BTreeMap<Feature, FeatureState> }` with `get(Feature) -> FeatureState` (missing or reserved → `Off`), `validated(self) -> Result<Self, LinkError>` (quiet only `On|Off`, ask only `Ask|Off`)
  - `PolicyDoc::phone_defaults()` (clipboard On, files Ask, handoff Ask, ring On) and `PolicyDoc::computer_defaults()` (all Off)
  - `pub enum Direction { Give, Receive }`; `pub fn allowed(local: &PolicyDoc, remote: &PolicyDoc, f: Feature) -> FeatureState` (Off if either side Off; else the stricter of the two, Ask beats On)
  - Policy lane messages: `pub struct PolicyUpdate(pub PolicyDoc)`; `pub fn send_policy(stream: &mut Stream, doc: &PolicyDoc)`, `pub fn recv_policy(stream: &mut Stream, t: Duration) -> Result<PolicyDoc, LinkError>`

- [ ] **Step 1: Failing tests**: `gate_requires_both_sides` (table over On/Off/Ask × On/Off/Ask for each Phase 1 feature), `reserved_features_read_off`, `quiet_feature_rejects_ask`, `defaults_match_spec`, `policy_doc_json_shape` (serde_json of phone defaults equals `{"version":1,"features":{"clipboard":"on","files":"ask","handoff":"ask","ring":"on"}}`).
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(link): add feature policy and two-sided gate`.

### Task 8: Clipboard protocol

**Files:** Create `crates/carrova-link/src/clipboard.rs`; Test `crates/carrova-link/tests/clipboard/main.rs`

**Interfaces:**
- Produces: `pub enum ClipItem { Text(String), Png(Vec<u8>) }`; `pub const MAX_CLIP_TEXT: usize = 1 << 20; pub const MAX_CLIP_PNG: usize = 10 << 20;`; `ClipItem::check(&self) -> Result<(), LinkError>` (`TooBig`); `pub fn send_clip(conn: &Connection, item: &ClipItem) -> Result<(), LinkError>`; `pub fn recv_clip(stream: Stream) -> Result<ClipItem, LinkError>` (enforces limits on the receive side before buffering beyond the cap); `pub struct ClipHistory` (cap 20, newest first) with `push`, `items`, `clear`.

- [ ] **Step 1: Failing tests**: `text_roundtrip`, `png_roundtrip`, `oversize_text_refused_by_sender`, `oversize_rejected_by_receiver_even_if_sender_lies` (raw frame with declared size > cap → `TooBig`, nothing buffered past cap), `history_keeps_twenty_newest`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(link): add clipboard protocol`.

### Task 9: File offer and transfer protocol

**Files:** Create `crates/carrova-link/src/file.rs`, `src/name.rs`; Test `crates/carrova-link/tests/file/main.rs`, `tests/name/unit.rs`

**Interfaces:**
- Produces:
  - `pub struct FileOffer { pub id: u64, pub name: String, pub size: u64, pub mime: Option<String> }`; `pub enum OfferAnswer { Accept { resume_from: u64 }, Decline, Expired }`
  - `pub fn safe_file_name(raw: &str) -> String` (strip path separators and control chars, trim dots/spaces, Windows reserved names prefixed `_`, cap 200 bytes on a char boundary keeping the extension, empty → `"file"`)
  - `pub trait FileSink { fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()>; fn commit(self: Box<Self>) -> io::Result<PathBuf>; fn abort(self: Box<Self>); }`
  - `pub fn offer_file(conn: &Connection, offer: &FileOffer, src: &mut dyn Read + Seek, on_progress: &mut dyn FnMut(u64)) -> Result<(), LinkError>` (1 MiB chunks, BLAKE3 per chunk header)
  - `pub fn receive_file(stream: Stream, decide: &mut dyn FnMut(&FileOffer) -> OfferAnswer, sink: Box<dyn FileSink>, on_progress: &mut dyn FnMut(u64)) -> Result<PathBuf, LinkError>`

- [ ] **Step 1: Failing tests**: `safe_file_name_cases` (`"../x"→"x"`, `"a/b\\c"→"abc"`, `"con.txt"→"_con.txt"`, `"\0name"→"name"`, `"."→"file"`, 300-char name → ≤200 bytes ends with `.pdf`), `file_roundtrip_hash_matches`, `declined_offer_writes_nothing`, `corrupt_chunk_fails` (flip a byte → `Protocol`), `interrupted_transfer_leaves_no_final_file` (drop sender mid-way → `abort` called, `commit` not), `resume_from_offset`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(link): add file offer and transfer protocol`.

### Task 10: Handoff and Ring protocols

**Files:** Create `crates/carrova-link/src/handoff.rs`, `src/ring.rs`; Test `crates/carrova-link/tests/handoff/main.rs`, `tests/ring/main.rs`

**Interfaces:**
- Produces: `pub enum Handoff { Url(String), File(FileOffer) }`; `pub fn check_url(url: &str) -> Result<(), LinkError>` (only `http`/`https`, ≤ 8 KiB, parsed with the `url` crate if already in the lock file, else a minimal scheme check); `pub fn send_handoff(conn, &Handoff) -> Result<OfferAnswer, LinkError>`; `pub fn recv_handoff(stream) -> Result<Handoff, LinkError>`; `pub enum RingMsg { Start { seconds: u16 }, Stop }`; `pub const RING_SECONDS: u16 = 30;`; `pub fn send_ring(conn, RingMsg)`, `pub fn recv_ring(stream) -> Result<RingMsg, LinkError>`.

- [ ] **Step 1: Failing tests**: `handoff_rejects_non_http` (`file:///etc/passwd`, `javascript:alert(1)`, `data:…`, `HTTP://ok.example` accepted case-insensitively), `handoff_url_roundtrip`, `ring_start_stop_roundtrip`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(link): add handoff and ring protocols`.

---

## Part D — Platform

### Task 11: Platform methods for Phase 1 (Linux + fake)

**Files:**
- Modify: `crates/carrova-platform/src/platform.rs` (trait), `src/types.rs`, `src/linux/*`, `src/fake/*`, `src/lib.rs` re-exports
- Create: `crates/carrova-platform/src/linux/clipboard.rs`, `src/linux/local_socket.rs`
- Test: `crates/carrova-platform/tests/clipboard/main.rs` (fake), `tests/local_socket/main.rs` (Linux), `tests/linux/clipboard/unit.rs` (MIME-hint parsing)

**Interfaces (all with default bodies returning `PlatformError::Unsupported`, overridden on Linux and in `FakePlatform`):**
- `fn clipboard_read(&self) -> Result<Option<ClipContent>>`; `fn clipboard_write(&self, c: &ClipContent) -> Result<()>`; `fn watch_clipboard(&self, on_change: Box<dyn Fn() + Send>) -> Result<ClipboardWatch>`
- `pub struct ClipContent { pub kind: ClipKind, pub bytes: Vec<u8>, pub sensitive: bool }`, `pub enum ClipKind { Text, Png }`
- `fn downloads_dir(&self) -> Result<PathBuf>`; `fn open_url(&self, url: &str) -> Result<()>`; `fn open_path(&self, p: &Path) -> Result<()>`
- `fn ring(&self, seconds: u16) -> Result<()>`; `fn stop_ring(&self) -> Result<()>` (Linux: Unsupported — computers cannot ring)
- `fn multicast_lock(&self) -> Result<MulticastGuard>` (default: no-op guard, Ok)
- `fn bind_local_socket(&self, path: &Path) -> Result<LocalListener>`; `fn connect_local_socket(&self, path: &Path) -> Result<LocalStream>` (Unix socket, mode `0600`, peer-uid check equals `current_uid()`; Windows Unsupported for now)
- `fn link_keepalive(&self) -> Duration` (default 15 s)
- Linux clipboard: Wayland via `wl-clipboard-rs`, X11 via `arboard` (pick the maintained crates; must list MIME types so `x-kde-passwordManagerHint` = `secret` sets `sensitive`). Watch = poll every 500 ms comparing a BLAKE3 of the content; `// shortcut: polling, switch to data-control events when wl-clipboard-rs exposes them`.

- [ ] **Step 1: Failing tests**: fake `clipboard_roundtrip_and_watch_fires`, `password_hint_marks_sensitive` (unit, MIME list → `sensitive = true`), `local_socket_mode_0600`, `local_socket_rejects_other_uid` (fake uid check via the injected comparator), `default_methods_unsupported` (a minimal impl gets `Unsupported`).
- [ ] **Step 2–4:** FAIL → implement → PASS (`cargo test -p carrova-platform --features fake`).
- [ ] **Step 5: Commit** `feat(platform): add clipboard, open, local socket methods`.

### Task 12: Android target compiles

**Blocked by Task 0 sign-off (ADR 0003).**

**Files:**
- Modify: `crates/carrova-platform/src/os.rs` (`OsKind::Android`, `ALL`, `display_name` "Android", `permission_model` Unix), `src/lib.rs` (`#[cfg(target_os = "android")] mod android;` and `current()` arm), `crates/carrova-platform/Cargo.toml` (`[lints]` per ADR 0003; `jni`, `ndk-context` under `[target.'cfg(target_os = "android")'.dependencies]`), `rust-toolchain.toml` (targets add `aarch64-linux-android`, `x86_64-linux-android`), `justfile` (`android-check`)
- Create: `crates/carrova-platform/src/android/{mod.rs,jni.rs}` (`AndroidPlatform` with `state_dir`/`resolve_root` from the app's files dir, other methods default)
- Test: `crates/carrova-platform/tests/os/main.rs` (`android_os_kind_roundtrip`), regenerate TS bindings (`OsKind` gains `"android"`), the front-end `osName` helper in `apps/desktop/src/screens/send/` handles `'android'`.

- [ ] **Step 1:** failing test `android_os_kind_roundtrip` (`"android"` serde, `display_name() == "Android"`).
- [ ] **Step 2:** implement; add `just android-check` = `cargo check -p carrova-platform -p carrova-core -p carrova-link --target aarch64-linux-android` (needs `ANDROID_NDK_HOME`; recipe prints a clear message if missing).
- [ ] **Step 3:** run `just android-check` → compiles; `just bindings` → `OsKind.ts` updated; `just ci` green.
- [ ] **Step 4: Commit** `feat(platform): add Android target and OS kind`.

### Task 13: Android platform methods via JNI + process-lifetime spike

**Files:**
- Modify: `crates/carrova-platform/src/android/{mod.rs,jni.rs}`; create `src/android/{clipboard.rs,files.rs,ring.rs,net.rs}`
- Create Kotlin glue (after Task 21 Step 1 generates the Android project): `apps/desktop/src-tauri/gen/android/app/src/main/java/org/byteveda/carrova/platform/{Clipboard.kt,Files.kt,Ring.kt,Net.kt}`
- Test: `crates/carrova-platform/tests/android/unit.rs` (pure helpers: MIME → `ClipKind`, sensitive-extra parsing); on-device checks listed in Step 3

**Interfaces:** Android overrides of `clipboard_read/write` (sensitive from `ClipDescription.EXTRA_IS_SENSITIVE`), `downloads_dir` + a MediaStore-backed `FileSink` constructor `android_downloads_sink(name, mime)`, `open_url`, `open_path`, `ring`/`stop_ring` (`AudioAttributes.USAGE_ALARM`, max volume, 30 s), `multicast_lock` (`WifiManager.MulticastLock` held by the guard), `link_keepalive` = 45 s. `watch_clipboard` stays Unsupported (Android blocks background reads).

- [ ] **Step 1: Spike (time-boxed, 1 day):** prove on a device/emulator that the Rust supervisor thread keeps running with the activity destroyed while `LinkService` runs, and record how it starts after boot. Write the result into ADR 0003 "Consequences" (commit `docs(adr): record Android lifetime spike`). If neither ADR mechanism works, STOP and ask the user.
- [ ] **Step 2:** failing unit tests for the pure helpers → implement JNI calls + Kotlin glue → unit tests PASS; `just android-check` green.
- [ ] **Step 3:** on emulator (API 34): clipboard write then read returns the same text; a sensitive clip reads `sensitive = true`; `ring(30)` plays and `stop_ring` stops; a file written via the MediaStore sink appears in Downloads.
- [ ] **Step 4: Commit** `feat(platform): implement Android clipboard, files, ring`.

---

## Part E — Core

### Task 14: Policy store, QR invite and pair-only in core

**Files:**
- Create: `crates/carrova-core/src/ecosystem/{mod.rs,policy.rs,invite.rs,pair.rs}`; modify `lib.rs` (`pub mod ecosystem`), `devices/mod.rs` (`forget`/`forget_all`/`reset_identity` delete policy files), `error/mod.rs` (`CoreError::KeyMismatch`, `CoreError::InvalidInvite`, `CoreError::NotLinked`)
- Test: `crates/carrova-core/tests/ecosystem/{main.rs,policy.rs,invite.rs,pair.rs}`

**Interfaces:**
- Consumes: `carrova_link::policy::{PolicyDoc, Feature, FeatureState}`, transport `pair_only`, `Listener::accept_pair_only`, `KnownDevices::{open, device_key, remember}`.
- Produces:
  - `pub struct PolicyStore` with `open(p: &dyn Platform) -> Result<Self>`, `get(&self, p, id: &DeviceId) -> Result<PolicyDoc>` (missing file → `computer_defaults()` for computers, phone defaults are written at pairing), `set(&self, p, id, f: Feature, s: FeatureState) -> Result<PolicyDoc>` (validated), `put(&self, p, id, &PolicyDoc)`, `remove(&self, p, id)`; files at `devices/policy/<hex>.json`, `0600`, `write_atomic`.
  - `pub struct PairInvite { pub ticket: Ticket, pub key: DevicePublic }` with `to_uri(&self) -> String` and `FromStr` for `carrova://pair?v=1&t=…&k=…` (base32 = `data_encoding::BASE32_NOPAD`; unknown `v` → `InvalidInvite`).
  - `pub fn offer_pairing(engine: &Engine, on_code: impl FnOnce(PairInvite)) -> Result<DeviceRecord>` (listener side: bind, advertise code, `accept_pair_only`, remember with `PolicyDoc::phone_defaults()` if the peer's OS is Android else `computer_defaults()`).
  - `pub fn accept_invite(engine: &Engine, invite: &PairInvite) -> Result<DeviceRecord>` (pinned) and `pub fn link_with_code(engine: &Engine, ticket: &Ticket) -> Result<DeviceRecord>` (unpinned).
  - The record's `name`/`os` come from `PairedPeer` (Task 3). This side's `PeerInfo` = `announced_name(p)` + `p.os()`.

- [ ] **Step 1: Failing tests**: `policy_file_mode_and_shape` (0600, JSON per spec), `policy_set_rejects_ask_for_quiet`, `forget_removes_policy`, `invite_uri_roundtrip`, `invite_rejects_unknown_version`, `pair_with_invite_remembers_both_sides` (two `Side`s from `tests/support/lan.rs`, loopback ticket), `pair_with_wrong_key_fails_key_mismatch`, `existing_known_computer_defaults_all_off`.
- [ ] **Step 2–4:** FAIL → implement → PASS (`cargo test -p carrova-core ecosystem`).
- [ ] **Step 5: Commit** `feat(core): add device policy store and pairing invites`.

### Task 15: Link supervisor and events

**Files:**
- Create: `crates/carrova-core/src/ecosystem/{supervisor.rs,peer.rs,dial.rs,asks.rs,features.rs}`; modify `session/events.rs`, `engine/mod.rs` (accessor only if needed)
- Test: `crates/carrova-core/tests/ecosystem/supervisor.rs` (two `Side`s, presence replaced by an injected `PeerSource`)

**Interfaces:**
- Produces:
  - `pub trait PeerSource: Send + Sync { fn seen(&self) -> Vec<SeenPeer>; }` with `pub struct SeenPeer { pub id: DeviceId, pub addr: SocketAddrV4 }`; default impl backed by `carrova_transport::browse` + known tags (`known_by_recent_use`).
  - `pub struct Supervisor` — `start(engine: Engine, source: Arc<dyn PeerSource>) -> Result<Self>`, `stop(self)`; dial rule: lower `DevicePublic` bytes dials; dial only if some feature is not `Off` locally; backoff 1 s→60 s; peer unseen 60 s → close; network change (`lan_ipv4_interfaces` diff) resets backoff.
  - Intents (all `&self`, return `Result<(), CoreError>`): `send_clipboard(&DeviceId)`, `send_file(&DeviceId, &Path)`, `handoff(&DeviceId, Handoff)`, `ring(&DeviceId)`, `stop_ring(&DeviceId)`, `answer(AskId, Answer)` (`Answer::{Accept, Decline}`), `set_feature(&DeviceId, Feature, FeatureState)` (persists via `PolicyStore` and pushes `PolicyUpdate` live), `ping(&DeviceId) -> Result<Duration>`, `status() -> Vec<LinkedPeer>`.
  - `pub struct LinkedPeer { pub id: DeviceId, pub name: String, pub os: OsKind, pub state: PeerState, pub local: PolicyDoc, pub remote: Option<PolicyDoc> }`, `pub enum PeerState { Linked, Connecting, Away }`.
  - Desktop clipboard auto-send: when `clipboard` allowed and `watch_clipboard` is supported, changes are sent to every linked peer allowing it; sensitive skipped (`Event::ClipSkipped { reason: Sensitive|TooBig }`).
  - Received clipboard → `clipboard_write` + history; files → `downloads_dir` sink via `FileSink` impl in core (`fsx`-based temp file + `rename_noreplace`, non-clobbering ` (2)` suffix); handoff URL → `open_url` after accept; ring → `platform.ring(RING_SECONDS)`.
  - New `Event` variants: `LinkUp { device: DeviceId }`, `LinkDown { device: DeviceId, retrying: bool }`, `PolicyChanged { device: DeviceId }`, `AskReceived { ask: AskId, device: DeviceId, kind: AskKind }` (`AskKind::{File { name, size }, Handoff { url_host: Option<String>, file: Option<String> }}`), `AskExpired { ask }`, `ClipReceived { device }`, `ClipSent { device }`, `ClipSkipped { reason }`, `FileProgress { device, id, done, total }`, `FileSaved { device, id, path }`, `FileFailed { device, id, reason }`, `HandoffOpened { device }`, `RingStarted { device }`, `RingStopped { device }`. (No contents in events except file names/hosts needed by the UI; never logged.)
  - Ask expiry 60 s → `OfferAnswer::Expired` to the sender.

- [ ] **Step 1: Failing tests** (two sides, loopback, `FakePlatform` clipboard): `link_up_when_feature_on`, `no_dial_when_all_off`, `duplicate_dial_keeps_one_connection`, `reconnect_after_drop`, `forget_drops_live_link`, `policy_change_applies_live`, `clipboard_flows_only_when_both_on`, `sensitive_clip_skipped`, `file_ask_accept_saves_to_downloads`, `file_ask_expires_after_timeout` (inject a 200 ms expiry via a test-only config), `handoff_url_opens_after_accept` (fake records `open_url`), `ring_reaches_peer`, `unknown_lane_does_not_drop_link`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(core): add link supervisor with clipboard, files, handoff, ring`.

### Task 16: One supervisor per user + control socket

**Files:** Create `crates/carrova-core/src/ecosystem/control.rs`; Test `crates/carrova-core/tests/ecosystem/control.rs`

**Interfaces:**
- Produces: `pub enum ControlRequest { Status, Ping(DeviceId), SendClipboard(DeviceId), SendFile(DeviceId, PathBuf), Handoff(DeviceId, Handoff), Ring(DeviceId), SetFeature(DeviceId, Feature, FeatureState) }`; `pub enum ControlResponse { Status(Vec<LinkedPeer>), Pong(u64 /* ms */), Done, Failed(String) }` (CBOR, u32 length prefix, cap 64 KiB); `pub struct ControlServer` `bind(engine, &Supervisor) -> Result<Self>` (takes `<state>/link.lock` via `lock_exclusive`, binds `<state>/link.sock`); `pub enum SupervisorHandle { Local(Supervisor), Remote(ControlClient) }` with `pub fn attach_or_start(engine: Engine) -> Result<SupervisorHandle>` (lock held elsewhere → client; else start local and serve).

- [ ] **Step 1: Failing tests**: `second_supervisor_becomes_client`, `client_status_matches_server`, `stale_socket_file_replaced` (socket file exists, no lock holder → bind succeeds), `oversize_request_rejected`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(core): run one link supervisor per user`.

---

## Part F — Front-ends

### Task 17: View-models and IPC contract

**Files:** Create `crates/carrova-view/src/ecosystem.rs`; modify `src/lib.rs`, `src/ipc/mod.rs` (`BridgeEvent::Link { event }`), `tests/export_bindings/main.rs`; Test `crates/carrova-view/tests/ecosystem/main.rs`

**Interfaces:**
- Produces (all TS-exported): `LinkedDeviceView { id, name, os, fingerprint, state: "linked"|"connecting"|"away", features: Vec<FeatureRowView> }`, `FeatureRowView { feature, group: "quiet"|"ask", local: FeatureState, remote: Option<FeatureState>, effective: FeatureState }`, `PairInviteView { uri, qr_svg: String, code: CodeView, expires_in_s: u32 }` (QR rendered with the `qrcode` crate to SVG, new dependency in `carrova-view` only), `AskView { ask_id, device_name, kind, file_name, size, url_host, expires_in_s }`, `ClipHistoryView { items: Vec<ClipItemView> }` (`ClipItemView { kind: "text"|"image", preview: String /* first 120 chars */, at }`), `TransferView { id, device_name, file_name, done, total, state }`; `fn linked_device_views(&[LinkedPeer]) -> Vec<LinkedDeviceView>`; `fn feature_rows(local, remote) -> Vec<FeatureRowView>` (Phase 1 features only, spec order: clipboard, ring, files, handoff).
- [ ] **Step 1: Failing tests**: `rows_show_remote_off`, `effective_matches_gate`, `invite_view_has_svg_and_uri`, `history_preview_truncates_120`.
- [ ] **Step 2–4:** FAIL → implement → PASS; `just bindings`; `just bindings-check` clean.
- [ ] **Step 5: Commit** `feat(view): add ecosystem view-models`.

### Task 18: CLI

**Files:** Create `apps/cli/src/link.rs`, `apps/cli/src/render/link.rs`; modify `apps/cli/src/main.rs` (`Command::Link`, `Command::Clip`, `Command::File`, `Command::Handoff`), `apps/cli/src/devices.rs` (`DevicesCommand::Link { target: String }`, `DevicesCommand::Features { device, set: Vec<String> }`); Test `apps/cli/tests/link/main.rs`, `apps/cli/tests/devices/main.rs`

**Interfaces:** exactly the commands in the Phase 1 spec "CLI" block; all via `attach_or_start`; `--json` like existing `devices`. `devices features` parses `feature=state` (bad pair → exit 2 with "expected <feature>=<on|off|ask>").
- [ ] **Step 1: Failing tests**: `devices_features_set_and_show`, `features_bad_pair_exit_2`, `link_status_json_shape`, `clip_send_requires_linked_device` (unknown → `UnknownDevice` message), `devices_link_with_ticket_remembers` (loopback two-process test using `carrova link serve` in a temp state dir).
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(cli): add link, clip, file and handoff commands`.

### Task 19: Desktop bridge

**Files:** Create `apps/desktop/src-tauri/src/commands/ecosystem.rs`, `src/ops/ecosystem.rs`; modify `src/lib.rs` (start `attach_or_start` in `.setup` next to `start_presence`; register commands), `src/tray.rs` (menu: "Send clipboard to…" submenu of linked devices, "Ring Pixel 8"), `src/state.rs` (hold `SupervisorHandle`); Test `apps/desktop/src-tauri/tests/ops/ecosystem.rs`

**Interfaces:** commands `linked_devices`, `pair_invite` (starts `offer_pairing`, emits `BridgeEvent::Link` with `PairInviteView`, cancellable), `link_computer(ticket)`, `set_feature(id, feature, state)`, `answer_ask(ask_id, accept)`, `send_clipboard(id)`, `send_files(id, paths)`, `handoff_url(id, url)`, `ring(id)`, `clip_history`, `clear_clip_history`. Each is a thin wrapper over `ops::ecosystem` → core. Events forwarded through the existing sink/throttle.
- [ ] **Step 1: Failing tests** in `tests/ops/ecosystem.rs`: `ops_set_feature_persists`, `ops_answer_unknown_ask_errors`, `ops_linked_devices_maps_views`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(desktop): bridge ecosystem commands and events`.

### Task 20: Desktop UI (Phase 1 screens from the desktop mockup)

**Files:** `apps/desktop/src/ipc/{port.ts,tauri.ts,fake/*,fixtures/*}` (new port methods + fakes), `src/flows/ecosystem/{useLinkedDevices.ts,useAsks.ts,useClipHistory.ts,useTransfers.ts}`, `src/screens/devices/{AddPhoneDialog.tsx,LinkComputerDialog.tsx,DevicePage.tsx,FeatureRow.tsx,ForgetDialog.tsx}`, `src/screens/home/` (ecosystem glance), `src/screens/nearby/` (linked phones/computers, "Add a phone", "Link a computer"), `src/screens/asks/{AskToast.tsx,ClipToast.tsx,TransferToast.tsx}`, `src/screens/clipboard/ClipHistorySheet.tsx`, `src/screens/settings/` (Features, Linked devices, Clipboard sections), `src/app/nav/navMachine.ts` (overlay `add-phone`, `link-computer`, view `device:<id>`), `src/i18n/en/ecosystem.ts`; Tests mirror under `apps/desktop/tests/{screens,flows}/…`, Playwright `apps/desktop/e2e/ecosystem.spec.ts` (fake port).

Screens and states = desktop mockup groups **Devices, Home, Clipboard, Files, Handoff, Settings, Tray** limited to Phase 1 features (hide Notifications/Messages/Calls/Screen/Keyboard rows). Sidebar stays Home, Nearby, History, Apps, Settings in Phase 1 (Messages/Calls entries arrive in Phase 2).

- [ ] **Step 1: Failing tests**: `DevicePage shows "Off on Pixel 8" when remote off`, `AddPhoneDialog renders QR svg and code, shows expired state`, `AskToast accept calls answerAsk(true)`, `ClipHistorySheet lists newest first and clears`, `Nearby shows linked phone with Linked badge`, e2e `pair a phone then send clipboard (fake port)`.
- [ ] **Step 2–4:** FAIL → implement (reuse existing components/tokens; quiet-instrument rules: nothing said twice) → `just ui-lint ui-test` PASS.
- [ ] **Step 5: Commit** (split): `feat(desktop): add device pages and pairing dialogs`, `feat(desktop): add ask, clipboard and transfer toasts`, `feat(desktop): add ecosystem settings sections`.

### Task 21: Android app

**Files:** `apps/desktop/src-tauri/Cargo.toml` (`[lib] crate-type = ["staticlib","cdylib","rlib"]`, `tauri-plugin-barcode-scanner` for mobile), `src/lib.rs` (`#[cfg_attr(mobile, tauri::mobile_entry_point)]` on `run`, the one exception signed off in ADR 0003), `apps/desktop/src-tauri/gen/android/**` (generated by `just tauri android init`, committed), Kotlin `LinkService.kt`, `BootReceiver.kt`, `ClipboardTile.kt`, `ShareActivity.kt` (share target → Rust command via the bridge), `AndroidManifest.xml` (permissions: `INTERNET`, `ACCESS_WIFI_STATE`, `CHANGE_WIFI_MULTICAST_STATE`, `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_CONNECTED_DEVICE`, `POST_NOTIFICATIONS`, `RECEIVE_BOOT_COMPLETED`, `CAMERA`), `tauri.android.conf.json` (identifier, minSdk 29); React phone layouts `apps/desktop/src/mobile/**` (onboarding, home, device page, clipboard send sheet/history, files share/progress/ask, handoff, activity, settings) selected when `about.os === 'android'`; `justfile` (`android-dev`, `android-build` → signed APK), `.github/workflows/release.yml` (APK job, after user OK); Tests `apps/desktop/tests/mobile/**`.

- [ ] **Step 1:** `just tauri android init`; commit generated project `chore(android): generate Tauri Android project`.
- [ ] **Step 2: Failing UI tests** (Vitest, fake port with `os: 'android'`): `Onboarding scan → linking → choose what to share shows Phase 1 rows only`, `Home shows linked computer and Send clipboard`, `Choose what to share preselects clipboard and ring`, `Not on Wi-Fi state`.
- [ ] **Step 3:** implement React mobile screens + Kotlin service/tile/share/boot (Kotlin calls only Tauri commands; no decisions).
- [ ] **Step 4:** `just ui-test` PASS; `just android-build` produces an APK; on emulator: install, pair with a desktop running on the host (emulator bridged networking or a real device on the LAN), clipboard both ways.
- [ ] **Step 5: Commit** (split): `feat(android): add link service, tile and share target`, `feat(android): add phone screens`, `build(android): add APK build recipes`.

### Task 22: Docs, e2e and exit gate

**Files:** `CLAUDE.md` (Status: "Ecosystem Phase 1 implemented, exit gate pending"; v1 scope line; layout adds `crates/carrova-link`, `apps/desktop/src/mobile`), `docs/PLAN.md` (new "Ecosystem" section with the three phases, status, link to specs/plan), `tests/e2e/` (new `just e2e --link`: two containers, pair-only via ticket, CLI clip/file/handoff round trip), `tasks/todo.md` (exit-gate checklist), `docs/mockups/gui-ecosystem-android.html` (add "Allow restricted settings" state for Phase 2 — note only, no Phase 1 code).

- [ ] **Step 1:** write `just e2e --link`; run → PASS.
- [ ] **Step 2:** update docs.
- [ ] **Step 3:** `just ci` → green; paste output into the PR description draft.
- [ ] **Step 4: Commit** `docs: record ecosystem phase 1 status`, `test(e2e): add linked-device container test`.
- [ ] **Step 5:** hand the real-hardware exit gate (Phase 1 spec "Acceptance → Real hardware") to the user as a checklist in `tasks/todo.md`. Do not mark Phase 1 complete until the user reports it passed.

---

## Dependency graph (for parallel waves)

```
T0 ──► T1 ─► T2 ─► T3 ─► T4 ─► T5 ─► T6 ─► T7 ─┬─► T8 ─┐
 │                                              ├─► T9 ─┼─► T15 ─► T16 ─┬─► T18
 │                                              └─► T10┘   ▲            ├─► T19 ─► T20
 └──► T12 ─► T13 (needs T21 step 1)            T11 ───────┘            └─► T17 ─┘
                                               T14 (needs T3, T7) ─► T15
T21 (needs T13, T17, T19) ─► T22
```

Suggested waves: [T0] → [T1] → [T2] → [T3, T11, T12] → [T4] → [T5, T14-prep] → [T6] → [T7] → [T8, T9, T10] → [T14] → [T15] → [T16, T17] → [T18, T19] → [T20, T21-step1, T13] → [T21] → [T22].
