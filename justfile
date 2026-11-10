# Carrova — single entry point for every workflow. `just --list` for help.

set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# Fast inner loop: check + unit tests for one crate (or whole workspace)
dev crate="":
    if [ -n "{{crate}}" ]; then cargo check -p {{crate}} --all-targets && cargo test -p {{crate}}; else cargo check --workspace --all-targets; fi

# Build every crate and the CLI
build:
    cargo build --workspace

# Unit + integration tests (all features so test doubles compile)
test:
    cargo test --workspace --all-features

# Recipe fixture tests; optionally one app: `just fixtures git`
fixtures app="":
    CARROVA_FIXTURE_FILTER="{{app}}" cargo test -p carrova-recipe --test fixtures -- --nocapture

# Validate every recipe in recipes/ (schema + policy issues)
recipes-validate:
    cargo run -q -p carrova-cli -- recipes validate recipes

# Ubuntu system packages for building the desktop crate (CI + fresh dev machines)
setup-ubuntu:
    sudo apt-get update && sudo apt-get install -y --no-install-recommends libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev libxdo-dev

# Format code
fmt:
    cargo fmt --all

# fmt check + clippy -D warnings + rustdoc -D warnings (broken/private doc links)
lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Everything CI would run
ci: lint build test fixtures bindings-check test-layout ui-lint ui-test

# End-to-end migration in clean Ubuntu containers (needs docker).
# `just e2e --archive FILE` imports an existing archive instead of a seeded one.
# `just e2e --lan [--only A|B1|B2|B3|C]... [--ubuntu-rmem]`: two containers on a
# Docker network migrate over LAN (pairing, interruption + resume, rollback).
e2e *args:
    tests/e2e/run.sh {{args}}

# Log machine load (loadavg, MemAvailable, free disk) every 30 s to /tmp/carrova-load.log
load-watch:
    while true; do printf '%s load=%s mem_mib=%s disk_mib=%s\n' "$(date +%T)" "$(cut -d' ' -f1-3 /proc/loadavg)" "$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)" "$(df -BM --output=avail . | tail -1 | tr -dc 0-9)" >> /tmp/carrova-load.log; sleep 30; done

# --- Desktop (Phase 4) ---

# Install front-end deps exactly as locked
ui-install:
    pnpm install --frozen-lockfile

# Run the desktop app with hot reload (Vite + tauri dev)
ui-dev:
    pnpm --filter desktop tauri dev

# Front-end lint: eslint + prettier --check + type check
ui-lint:
    pnpm --filter desktop exec eslint . --max-warnings 0
    pnpm --filter desktop exec prettier --check .
    pnpm --filter desktop exec tsc -b
    pnpm --filter desktop-e2e-app typecheck

# Front-end unit tests (Vitest, jsdom)
ui-test:
    pnpm --filter desktop exec vitest run

# Tauri CLI passthrough (CI's tauri-action calls `just tauri build`)
tauri *args:
    pnpm --filter desktop tauri {{args}}

# Release packages (AppImage + .deb) with the signed recipe bundle embedded.
# Dev keys by default; CI passes TAURI_SIGNING_* and its own signer.
package version="1" bundles="appimage,deb":
    just bundle {{version}}
    just bundle-sign
    just bundle-verify
    CARROVA_SHIPPED_INDEX_HASH="$(cargo xtask bundle pin --dir dist/recipes)" TAURI_SIGNING_PRIVATE_KEY="${TAURI_SIGNING_PRIVATE_KEY:-$(cat ~/.config/carrova-dev/updater.key)}" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-$(cat ~/.config/carrova-dev/updater.key.password)}" just tauri build --bundles {{bundles}} --config src-tauri/tauri.release.conf.json

# Alias kept for muscle memory: release packages
desktop-build: package

# Install the Playwright browser (+ system deps on CI)
ui-browsers:
    pnpm --filter desktop exec playwright install --with-deps chromium

# Browser smoke/e2e tests (Playwright) against the production build plus the
# fake port (`--mode e2e`; the shipped `pnpm build` contains no fake)
e2e-ui *args:
    pnpm --filter desktop exec tsc -b
    pnpm --filter desktop exec vite build --mode e2e --outDir dist-e2e --emptyOutDir
    pnpm --filter desktop exec playwright test {{args}}

# --- Recipe bundle (Phase 4) ---

# Build the recipe bundle from recipes/ into dist/recipes (version N must increase)
bundle version:
    cargo xtask bundle build --recipes recipes --out dist/recipes --version {{version}}

# Sign dist/recipes with a signer key + root-issued cert (defaults: dev keys)
bundle-sign signer="~/.config/carrova-dev/bundle-signer.key" cert="~/.config/carrova-dev/bundle-signer.cert.json":
    cargo xtask bundle sign --dir dist/recipes --signer {{signer}} --cert {{cert}}

# Verify dist/recipes against the embedded trust roots
bundle-verify:
    cargo xtask bundle verify --dir dist/recipes --roots keys/bundle-roots.json

# --- IPC bindings (Phase 4) ---

# Regenerate TS types for the desktop IPC from Rust (ts-rs)
bindings:
    cargo test -p carrova-view --features ts --test export_bindings -- --ignored

# Re-record the IPC fixtures the UI's fake port replays (real bridge runs)
ipc-fixtures:
    cargo test -p carrova-desktop record_ipc_fixtures -- --ignored
    pnpm --filter desktop exec prettier --write src/ipc/fixtures

# Fail if committed TS bindings are stale
bindings-check:
    #!/usr/bin/env bash
    set -euo pipefail
    tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
    CARROVA_BINDINGS_DIR="$tmp" cargo test -q -p carrova-view --features ts --test export_bindings -- --ignored
    diff -r --exclude=README.md "$tmp" apps/desktop/src/bindings

# Fail if release placeholders (OWNER/REPO slug, dev keys) remain
release-preflight:
    bash .github/scripts/release-preflight.sh

# Real desktop app smoke (tauri-driver + WebdriverIO under xvfb)
e2e-app *args:
    apps/desktop/e2e-app/run.sh {{args}}

# Recipe bundle update e2e (local release host, test root)
e2e-bundle *args:
    tests/e2e/bundle/run.sh {{args}}

# Tests live in tests/<module>/, never under src/
test-layout:
    scripts/check-test-layout.sh

# --- Commit conventions ---

# Lint commit messages in a range (default: commits on this branch not on main)
commitlint from="origin/main" to="HEAD":
    pnpm exec commitlint --from {{from}} --to {{to}} --verbose

# Lint one message (e.g. a PR title) read from stdin
commitlint-msg:
    pnpm exec commitlint --verbose

# Opt in to the local commit-msg hook (lints every commit before it is created)
hooks:
    git config core.hooksPath .githooks
