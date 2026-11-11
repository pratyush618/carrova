# Carrova

Open-source desktop app that moves app data (configs, keys, user data) between computers over the LAN. User picks exactly what moves. Apps are reinstalled, never copied. Rust engine, thin front-ends (CLI first, Tauri + React later). Apache-2.0.

- Design (what): `Porter — High-Level Design.pdf` (Porter was the working name)
- Plan (how / order): `docs/PLAN.md` — check current phase before starting work
- Working checklist: `tasks/todo.md` · Lessons: `tasks/lessons.md` (read at session start)

## Status

Phase 1 (Foundation) and Phase 2 (Archive migration) complete. Phase 3 (LAN transfer) complete (`carrova send|receive`, resume, `just e2e --lan`; hardware exit gate passed 2026-11-11). Phase 4 (Desktop app + recipe ecosystem) implemented: Tauri 2 app over a typed IPC contract, signed recipe bundle (Ed25519, delta updates, `carrova recipes update|status|rollback|reset`), 15 recipes, opt-in error reports, AppImage/.deb + updater, local git + CI workflows; v0.1 exit gate pending (user: clean-Ubuntu install, wizard migration, bundle update) after replacing release placeholders (`just release-preflight`). Phase 5 (cross-OS + WASM plugins) implemented; exit gate pending (user: Windows beta, macOS notarized build, cross-OS restores).

## Hard constraints

- **Monorepo.** Everything lives in this repo. Never propose splitting.
- **Git (user, 2026-11-10).** Remote `origin` = https://github.com/pratyush618/carrova, branch `main`. Commit and push only when the user asks; Conventional Commits (commitlint), split into focused commits, no AI attribution. Never force-push, hard-reset or clean (denied in `.claude/settings.json`). Use `cargo new --vcs none`.
- **Stay in the current phase.** Don't build ahead (no UI before Phase 4, no Windows/macOS/plugins before Phase 5). Stubs required by the plan are fine.
- **v1 scope:** Ubuntu → Ubuntu, developers, LAN + archive file.

## Layout (target)

```
crates/carrova-core        session state machine, emits typed events
crates/carrova-recipe      load/validate recipes, detect apps, resolve items
crates/carrova-manifest    OS-neutral transfer description, content-addressed chunks
crates/carrova-transport   mDNS/UDP discovery, QUIC streams, resume
crates/carrova-crypto      PAKE pairing, session keys, archive encryption
crates/carrova-archive     encrypted single-file export/import
crates/carrova-platform    the ONLY OS-aware crate (Linux adapter; Win/mac stubs)
crates/carrova-plugin      WASM runtime (stub until Phase 5)
crates/carrova-view        shared front-end view-models + desktop IPC contract (ts-rs bindings)
crates/carrova-bundle      signed recipe bundle: build, verify, update, store, resolve
crates/carrova-report      opt-in redacted error reports (consent, store, issue draft)
apps/cli                  carrova-cli
apps/desktop              React UI (src/, tests/, e2e/, e2e-app/) + src-tauri/ bridge
xtask/                    dev tooling: `cargo xtask bundle keygen|certify|build|sign|verify|pin`
keys/                     bundle trust roots (DEV root until replaced before release)
packaging/                packaging docs, desktop entry
recipes/schema/           versioned JSON Schema for recipes
recipes/<category>/<app>/ recipe.yaml + fixture/
tests/e2e/                container migration tests (+ bundle/ update e2e)
docs/                     PLAN.md, ADRs
```

## Boundary rules (never break)

1. No OS-specific code outside `carrova-platform`. No `cfg(target_os)`, `/proc`, XDG, `std::os::unix` elsewhere — add a trait method instead.
2. `src-tauri` holds no business logic. CLI and UI must behave identically.
3. Recipes are data only. No code, no absolute paths — only semantic roots (`user_config`, `user_data`, `cache`, `home`, `documents`) + relative paths.
4. Dependencies point down: front-ends → core → services → platform. Services never depend on core.

## Commands

```
just build      # cargo build --workspace
just test       # unit + integration
just fixtures   # recipe fixture tests
just lint       # fmt --check + clippy -D warnings + rustdoc -D warnings
just ci         # everything CI would run (run before calling work done)
```

`justfile` is the single entry point; add new workflows there, not as ad-hoc scripts.

## Rust conventions

- Edition 2024, stable toolchain (1.99+; pin via `rust-toolchain.toml` + workspace `rust-version` when scaffolding). `unsafe_code = "forbid"` unless a crate is explicitly allow-listed.
- Libraries: `thiserror` error enums, no `unwrap`/`expect`/`panic!` outside tests. CLI: `anyhow`.
- Logging via `tracing`. Never log file contents, secrets, keys, passphrases or pairing codes.
- Core emits typed events; front-ends render them. Front-ends never poke engine state.
- Crypto: audited crates only, no hand-rolled primitives. Any change to `carrova-crypto` or `carrova-transport` handshake needs explicit user sign-off.

## Security rules

- Receiver normalises every path, rejects escapes from semantic roots, never follows symlinks out of them.
- Sensitive items (SSH/GPG keys, tokens) default off and need explicit confirmation.
- Restore permissions exactly (e.g. `600` on SSH private keys).
- Nothing is written on the receiver before the dry-run plan is approved.
- Every apply is staged, verified, backed up and reversible.

## Testing

- **Tests live outside `src/`, grouped per module in `tests/<module>/` (user, 2026-11-08)** — every crate, `apps/cli`, `apps/desktop/src-tauri` and the desktop front-end. Rust: `tests/<module>/main.rs` (+ files beside it) = that module's public-API integration tests; `tests/<module>/unit.rs` = private-internals unit tests, wired from `src/<module>` with `#[cfg(test)] #[path = "…/tests/<module>/unit.rs"] mod tests;`. No inline `mod tests {}`, no test files under `src/`. Front-end: `apps/desktop/tests/<module>/…test.ts(x)` mirroring `src/<module>`; Playwright in `apps/desktop/e2e/`. Nested modules mirror the src tree (`src/a/b.rs` → `tests/a/b/unit.rs`); crate-root tests → `tests/lib/unit.rs`; shared helpers → `tests/support/mod.rs`. `just test-layout` enforces it.
- Every recipe ships with a fixture (fake home dir) and a passing fixture test.
- Session logic is tested via two engine instances in one process.
- Bug fix → regression test first.
- Done = `just ci` green, with output shown.
