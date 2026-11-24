# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

Website (`apps/site/`): React + TypeScript built with Vite (user choice, 2026-11-24), static output deployed to GitHub Pages from this repo. The desktop app is Tauri 2 + React (`apps/desktop`).

## Users

Everyday people getting a new computer who want their apps set up the way they had them, without copying folders by hand. Developers are a strong secondary audience (dotfiles, SSH/GPG keys, editor and shell configs). Android 13+ phone owners for the linked-phone features.

## Product Purpose

Carrova moves app data (settings, keys, user data) from one computer to another over the local network, or through an encrypted file. The user picks exactly what moves. Apps are reinstalled, never copied; their settings are waiting when they are. Success: the new computer feels like the old one, with nothing moved that the user did not choose.

## Positioning

Local-first and user-decides: no account, no cloud, end-to-end encrypted over the user's own network, a dry-run plan before anything is written, and every move can be undone. Free and open source (Apache-2.0).

## Operating Context

Two computers on the same network (or a USB stick with an encrypted file). Old computer: Send, tick apps, get a pairing code. New computer: Receive, type code, review plan, approve. A linked Android phone adds shared clipboard, file sending, notifications, texts, calls and Ring.

## Capabilities and Constraints

- Platforms: Linux (tested path: Ubuntu 22.04+, Debian 12+, Fedora, openSUSE; x86_64 + arm64). Windows 10/11, macOS 11+ and Android 13+ are beta: Windows builds unsigned, macOS not notarized.
- Releases on GitHub: https://github.com/pratyush618/carrova/releases (AppImage, .deb, .rpm, .dmg, .exe, .msi, .apk).
- Supported apps (recipes): VS Code, Zed, Neovim, Vim, Sublime Text, Obsidian, Git, GitHub CLI, Docker CLI, npm, Cargo, tmux, Bash, Zsh, fish, PowerShell, Starship, WezTerm, Windows Terminal, OpenSSH, GnuPG.
- Security: PAKE pairing code shown on both screens, end-to-end encrypted transfer (QUIC), encrypted archive files, Ed25519-signed recipe bundle, sensitive items (SSH/GPG keys, tokens) off by default, staged + verified + backed-up apply, undo.

## Brand Commitments

Name: Carrova. App icon: `apps/desktop/src-tauri/icons/icon.svg`. Voice: plain, calm, honest; short sentences; no hype, no dark patterns. App theme: dark Graphite tokens in `apps/desktop/src/theme/tokens.ts`.

## Evidence on Hand

Real app screenshots: `docs/screenshots/{home,send,review,done}.png` (2200×1440, dark). Supported-app list and security facts above. No testimonials, user counts, press or benchmarks exist; do not fabricate them.

## Product Principles

1. The user decides; defaults are suggestions in plain words.
2. Show consequences before acting; nothing is written without approval.
3. Private by default: local network, no account, no cloud.
4. Honest about state: beta platforms are labelled beta.

## Accessibility & Inclusion

WCAG 2.1 AA contrast (the app's tokens are measured against it); respect `prefers-reduced-motion`.
