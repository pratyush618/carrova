# Carrova — Implementation Plan

Source of truth for *what* Carrova is: `Porter — High-Level Design.pdf` (project root, Nov 8 2026; Porter was the working name).
This document covers *how and in what order* it gets built.

**Status:** Phase 1 complete (exit gate passed 2026-11-08) · Phase 2 complete (exit gate passed 2026-11-08) · Phase 3 complete (exit gate passed 2026-11-11) · Phase 4 implementation complete, v0.1 exit gate pending · Phase 5 implementation complete except notarization and Flathub, exit gate pending · Manual mode + "Send with Carrova" in progress (`feat/manual-mode`) · **Updated:** 2026-11-11

## Ground rules

- **One monorepo.** Crates, CLI, desktop app, recipes, docs and tests all live here. Release trains (app vs recipe bundle) are separated by tags and paths, never by repo.
- **Local git, no remote yet.** Workflows, CODEOWNERS and release pipeline exist in `.github/` (Phase 4); the user creates/pushes the GitHub repo. `just ci` reproduces every CI check locally.
- **Gated phases.** A phase starts only when the previous phase's exit gate passes. No dates.
- **Engine before UI.** Everything is driven and tested through `carrova-cli` before the desktop app exists.
- **Work method:** plan the whole phase, then implement independent tasks in parallel waves (≤3 subagents, disjoint files, shared `target/`, `jobs = 6`); integrate + `just ci` per wave.
- **Boundary rules hold from the first commit:** OS logic only in `carrova-platform`; no business logic in `src-tauri`; recipes are data only.

## Phase overview

| # | Phase | Outcome | Exit gate |
|---|-------|---------|-----------|
| 1 | Foundation | Workspace, platform layer, recipes, manifest; `carrova scan` works | Scan of the dev laptop is correct; starter recipe fixtures pass |
| 2 | Archive migration | Full offline migration via encrypted file, with plan, staging, apply, rollback | Archive round trip restores a working dev setup in a clean container; rollback verified |
| 3 | LAN transfer | Pairing, discovery, QUIC transport, resume; `carrova send` / `carrova receive` | Own laptop migrated to a new Ubuntu machine over LAN |
| 4 | Desktop app + recipe ecosystem | Tauri UI, signed recipe bundle, 15 dev recipes, packaging, git + CI | v0.1 public release candidate |
| 5 | Cross-platform + plugins | Windows and macOS adapters, WASM plugins, cross-OS restore | Windows beta stable; macOS builds notarized |

---

## Phase 1 — Foundation

**Goal:** the skeleton every later phase builds on, plus read-only inspection of a real machine.

**Deliverables**
- [x] Cargo workspace (edition 2024) with all crates as members: `carrova-core`, `carrova-recipe`, `carrova-manifest`, `carrova-transport`, `carrova-crypto`, `carrova-archive`, `carrova-platform`, `carrova-plugin` (stub), `apps/cli`
- [x] `justfile`: `dev`, `build`, `test`, `lint`, `fmt`, `ci`, `fixtures`
- [x] Workspace lints: clippy pedantic-lite set, `unsafe_code = "forbid"` outside allow-listed crates
- [x] `carrova-platform`: `Platform` trait — semantic location resolution, permissions (mode/owner), process detection, free space, snapshot capability probe
- [x] Linux adapter: XDG-based roots (`user_config`, `user_data`, `cache`, `home`, `documents`), `/proc` process listing, `statvfs`
- [x] Windows and macOS stub adapters (compile, return `Unsupported`) to keep the trait honest
- [x] `carrova-recipe`: recipe schema v1 (YAML + JSON Schema in `recipes/schema/`), loader, validator, app detection, item resolution against semantic roots, path confinement
- [x] `carrova-manifest`: types for manifest/items/files, content-addressed chunking and hashing, serialization format with version field
- [x] Unrecognised-data scan: folders under config/data roots matching no recipe, surfaced as raw items
- [x] Recipe fixture harness: fake home directory per recipe, capture assertions
- [x] Starter recipes (5): git, ssh, bash/zsh, VS Code, GnuPG
- [x] CLI: `carrova recipes validate`, `carrova scan [--json]`

**Decisions (made)**
- Hash/chunking: BLAKE3; FastCDC v2020, min 16 KiB / avg 64 KiB / max 256 KiB
- Manifest wire format: CBOR (ciborium), `format_version` checked before body decode; `deny_unknown_fields` (any new field bumps version). JSON only for `--json`
- Recipe schema v1: `recipes/schema/recipe.v1.json`; `export`/`import` reserved for v2

**Exit gate:** `carrova scan` on the dev laptop lists the right apps, items, sizes and flags; all starter recipe fixtures pass; `just ci` is green.

---

## Phase 2 — Archive migration

**Goal:** a complete, safe migration without the network. Archive mode exercises manifest, crypto, planning and restore, so Phase 3 only adds transport.

**Deliverables**
- [x] `carrova-core`: session state machine (`scan → select → plan → transfer → stage → apply → done | rolled_back`) emitting typed events
- [x] Session persistence in a local state directory; resumes after restart
- [x] Selection model: user decides everything — file-level tree, presets only pre-fill suggestions; sensitive items unticked + explicit confirmation; caches/logs/raw data selectable; force-include unmovable behind a warning
- [x] Dry-run plan: target paths, conflicts, overwrites, missing/older apps, space check; nothing written before approval
- [x] Conflict rule per item and per file: back up and replace (suggested), keep both, skip
- [x] User overrides with warnings: restore to a different location, ignore requires-closed, ignore free-space check
- [x] `carrova-crypto`: passphrase KDF + authenticated encryption for archives
- [x] `carrova-archive`: single-file encrypted export/import using the same manifest format as network mode
- [x] Staging area, hash verification, swap into place with backups
- [x] Rollback: record of every replaced file, one-step restore, rollback point kept until dismissed; automatic rollback when apply fails midway
- [x] Exact permission restore (e.g. `600` on SSH private keys)
- [x] Recipe schema v2 built-in handlers (`gpg-keyring`: `gpg --export`/`--import` + ownertrust); rollback = file backup of the handler's declared paths
- [x] "Requires closed": detect running apps, prompt or skip and report
- [x] File-change safety during capture: re-hash and re-capture changed files (snapshots deferred — capability still reported)
- [x] Receiver hardening: normalise paths, reject escapes from semantic roots, never follow symlinks out of them
- [x] Staging/temp wipe after apply; no plaintext secrets in logs
- [x] Structured per-session logs (`tracing`), exportable with secrets redacted
- [x] End-of-session checklist: moved, skipped (with reason), apps still to install
- [x] CLI: `carrova export`, `carrova import [--dry-run]`, `carrova rollback`, `carrova sessions`
- [x] Integration tests: full session between two engine instances in one process

**Decisions (made 2026-11-08, details in `tasks/todo.md`)**
- Product principle: Carrova offers options, the user makes every final call; defaults are visible, editable suggestions. Non-overridable rails: path confinement, hash verification, reversible apply, no secrets in logs
- Archive crypto: Argon2id (m=256 MiB, t=3, p=1) + XChaCha20-Poly1305 STREAM, 64 KiB segments, authenticated header — security review still required before exit
- Command-based items: recipe v2 names built-in Rust handlers (no commands in recipes); rollback backs up the handler's declared paths
- Snapshots deferred; re-hash is the capture safety path
- Manifest format v2 (`handler`, `artifacts`); state dir `$XDG_STATE_HOME/carrova`

**Exit gate:** export from the dev laptop, import into a clean Ubuntu container: SSH, git, shell and editor work; forced mid-apply failure rolls back cleanly; permissions verified. — **Passed 2026-11-08** (`just e2e --archive`, laptop archive: 7 SSH keys usable + 0600, `.ssh` 0700, ssh/git configs parse, bash/zsh start, VS Code settings present, 2 GPG public keys; dry run writes nothing; manual rollback and forced failure byte-identical).

---

## Phase 3 — LAN transfer

**Goal:** the same migration directly between two machines.

**Deliverables**
- [x] Discovery: mDNS plus UDP broadcast
- [x] Fallback typed ticket: the pairing code plus the sender's LAN address (`<code>@<ipv4>:<port>`), shown by the sender, typed on the receiver
- [x] PAKE pairing from the short code shown on both screens; session keys derived from it; no long-lived trust by default
- [x] `carrova-transport`: PAKE inside QUIC (TLS 1.3 over throwaway certs), key confirmation bound to that connection; multiplexed parallel streams
- [x] Receiver inventory exchange (OS, detected apps, free space; app versions unavailable in v1)
- [x] Conflict detection on the receiver (see open item below)
- [x] Priority ordering: small config items first, bulk data after
- [x] Resume from the last verified chunk across sleep, Wi-Fi drops and restarts; per-chunk re-request on hash mismatch
- [x] Disk-full handling: pause with a clear message
- [x] CLI: `carrova send`, `carrova receive`
- [x] E2E tests: two containers on a virtual network, including interruption and rollback

**Decisions (made 2026-11-08, details in `tasks/todo.md`)**
- PAKE: SPAKE2 (RustCrypto `spake2`) run inside QUIC over throwaway self-signed certs; key confirmation bound to the TLS exporter + both cert hashes; one guess per code (sender burns code on failure) — security review required before exit
- Pairing code: `<nameplate>-<word>-<word>` (EFF short wordlist), sender shows, receiver types; fallback ticket `<code>@<ip>:<port>`
- TLS backend: rustls + `ring`; tokio confined to `carrova-transport` (blocking API), core stays sync
- Conflict detection: sender sends manifest, receiver plans locally, returns only a plan summary
- Peer identity: per-session resume secret + peer cert hash, `0600`, deleted when the session closes
- Transfer: receiver pulls verified chunks in priority order over parallel streams; resume re-pulls only missing chunks

**Exit gate:** own laptop migrated to a new Ubuntu machine over LAN, including at least one forced interruption and resume.

---

## Phase 4 — Desktop app + recipe ecosystem

**Goal:** a releasable v0.1 for developers.

**Deliverables**
- [x] `apps/desktop`: Tauri 2 + React + TypeScript, pnpm workspace
- [x] `src-tauri` as a thin bridge: commands call `carrova-core`, core events stream to the UI; no business logic
- [x] Wizard flow (Send/Receive → pair → preset → plan → progress → checklist)
- [x] Advanced tree: every item with size, portability and sensitivity flags; unmovable items greyed with a one-line reason
- [x] i18n from day one
- [x] Recipe bundle: index + content-addressed recipe files, delta updates, precompiled detection index, verified vs community tiers, `recipes/` sharded by category
- [x] Bundle signing and client-side verification; engine declares accepted schema range
- [x] 15 developer recipes total, each with a fixture test
- [x] Move to git + GitHub: path-filtered Actions (engine, UI, recipes, release), CODEOWNERS trust tiers, recipe contribution guide (workflows + docs done; `git init`/GitHub repo by user)
- [x] Packaging: AppImage + `.deb`; `cargo install` for CLI; Tauri signed updater
- [x] Opt-in crash/error reporting
- [x] Playwright flows against the Tauri app (Playwright over the IPC fake + tauri-driver/WebdriverIO real-app smoke)

**Decisions (made 2026-11-08, details in `tasks/todo.md`)**
- Started before the Phase 3 hardware gate (user); gate runs in parallel
- Recipe hosting: GitHub Releases (`recipes-vN` tags); app checks on launch + daily
- Bundle signing: Ed25519 (`ed25519-dalek`); offline root key certifies a short-lived CI signing key; engine embeds root key list
- Git: local repo + workflow files this phase; user creates/pushes the GitHub repo
- New crates: `carrova-view` (shared front-end view-models), `carrova-bundle` (recipe bundle service), `xtask` (bundle tooling)
- Still open: final project name (HLD open question) — needed before public release

**Exit gate:** v0.1 release candidate: signed AppImage/.deb install on clean Ubuntu, wizard migration succeeds, recipe bundle update arrives without an app release.

---

## Phase 5 — Cross-platform + plugins

**Goal:** beyond Ubuntu, and beyond declarative recipes.

**Deliverables**
- [x] Windows adapter for `carrova-platform`; Windows paths in recipes; consumer recipes
- [ ] macOS adapter; notarized builds (adapter done; notarization CI wired, needs the Apple account)
- [x] `carrova-plugin`: WASM runtime with declared permission manifest shown to the user; core-maintainer review; signed plugins
- [x] Cross-OS restore driven by the portability flag and per-item semantic mapping
- [ ] CI build matrix: Ubuntu, Windows, macOS runners; Flathub listing (matrix done; Flathub deferred)

**Exit gate:** Windows beta stable; macOS notarized builds; at least one cross-OS restore path (e.g. VS Code settings Linux → Windows) verified end to end.

---

## Feature — Manual mode and "Send with Carrova" (in progress, `feat/manual-mode`)

**Goal:** the user picks exact files and folders (system picker, file-manager right-click, or `carrova send|export PATH...`); picks go through the same sensitive confirm, dry-run, staged apply and undo as Auto, and arrive unticked on the receiver with one "Accept all" for safe ones. Right-click entries on Linux, Windows and macOS are per-user and written only after the user agrees. Next release ships it for Linux; macOS/Windows release jobs stay off until signing is ready.

Spec: `docs/superpowers/specs/2026-11-11-manual-mode-design.md` · Plan: `docs/superpowers/plans/2026-11-11-manual-mode.md`

---

## Risks

| Risk | Mitigation |
|------|-----------|
| Snapshots rarely available (Ubuntu defaults to ext4) | Treat re-hash as the primary path; snapshot is an optimisation |
| Crypto mistakes | Use audited crates only; no custom primitives; security review gate in Phases 2 and 3 |
| Recipe quality at scale | Fixture test required per recipe; verified vs community tiers |
| Scope creep into UI before engine is solid | UI work blocked until Phase 3 gate passes |
