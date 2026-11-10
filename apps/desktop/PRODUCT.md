# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Tauri 2 desktop app (Linux webview, AppImage/.deb). Own design language, not native GTK; windowed desktop sizes, keyboard and pointer.

## Users

Developers on Ubuntu moving to a new or reinstalled machine (v1: Ubuntu → Ubuntu). Rare, high-stakes use: once or twice a year, at a moment when they want their environment back fast, and the payload includes SSH/GPG keys, tokens and dotfiles. Windows/macOS and non-developer users are later-phase audiences, not v1 design targets.

## Product Purpose

Carrova moves app data (configs, keys, user data) between two computers over the LAN, or through an encrypted archive file. Apps are reinstalled, never copied. Success: the user's new machine feels like theirs again, they chose exactly what moved, and nothing was written without their approval.

## Positioning

- **You pick exactly what moves.** The user decides everything; defaults are visible, editable suggestions, never silent decisions.
- **No cloud.** Direct computer-to-computer over the LAN, or an encrypted file. No server ever holds user data; the only hosted thing is a signed recipe bundle.
- **Reversible and safe.** Dry-run plan before any write, staged and verified apply, backups, undo afterwards.
- **Data, not apps.** Open, data-only recipes describe where each app keeps its data; apps themselves are reinstalled cleanly.

## Operating Context

- One binary on both machines; Send or Receive is chosen per session. CLI (`carrova`) and desktop app run the same engine and must behave identically.
- Flow: pair (short code, or typed address fallback) → receiver shares OS/apps/free space → sender scans and matches recipes → user selects → dry-run plan with approval gate → transfer (small configs first) → verify → apply with backups → checklist, undo.
- Archive mode replaces pairing with a file plus passphrase.
- History and undo of past applies; settings; opt-in redacted error reports.

## Capabilities and Constraints

- Recipes: 15 developer-app recipes in a signed bundle (Ed25519, delta updates, rollback).
- Sensitive items (SSH/GPG keys, tokens) start unticked and need explicit confirmation. Unmovable items are greyed with a reason, never hidden.
- Overrides (force unmovable, remap location, ignore requires-closed, ignore free space) are allowed behind a warning. Not overridable: path confinement, hash verification, reversible apply, no secrets in logs, sensitive-off default.
- Error-report consent asked once, default off, never re-prompted.
- Front-end holds no business logic; it renders typed engine events over an IPC contract.
- UI strings go through i18n (`src/i18n/en`), pseudo-locale exists.
- v1 non-goals: copying app binaries, cloud relay, OS-bound secrets (browser passwords, keyrings), Windows/macOS, cross-OS restore.

## Brand Commitments

- Name: Carrova (formerly working name "Porter"). Open source, Apache-2.0.
- Voice: plain language ("Your SSH keys", not file names or flags). Warmth allowed in success copy; security and warning text stays literal.
- No dark patterns: no confirmshaming, nagging, preselected risk, streaks/badges/return notifications, fake urgency or fake progress, obstruction. Cancel and Undo always reachable.

## Evidence on Hand

None yet: no users, testimonials, benchmarks or press. Future work must not invent any such claims. Internal Phase 3 performance notes (`docs/research/phase3-performance.md`) are engineering data, not marketing claims.

## Product Principles

1. The user decides; Carrova suggests. Every default is stated in words with a way to change it.
2. Hide controls, never consequences. Sensitive items, replaced files and overrides are always visible.
3. Nothing happens before approval, and everything after can be undone.
4. Trust is shown where it is asked for: encryption, approval gates and undo are stated at the moment they matter.
5. Rare use, high stakes: clarity on first sight beats power-user density.

## Accessibility & Inclusion

WCAG 2.2 AA: keyboard-complete, AA text contrast and 3:1 non-text contrast on controls, screen-reader labels, `prefers-reduced-motion` fallback for every decorative animation (WCAG 2.3.3 followed). axe checks run in Playwright.
