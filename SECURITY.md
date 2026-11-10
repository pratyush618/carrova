# Security policy

Carrova moves SSH/GPG keys, tokens and configs between machines. We treat
security reports as top priority.

## Reporting a vulnerability

**Do not open a public issue, PR or discussion.**

Report privately through GitHub: **Security → Report a vulnerability** on this
repository (GitHub Security Advisories). Include affected version/commit,
steps to reproduce, and impact. We aim to acknowledge within 3 working days and
to agree on a fix and disclosure timeline with you. We credit reporters in the
advisory unless you prefer otherwise.

## Supported versions

Until 1.0, only the latest release of the desktop app/CLI and the current
`recipes-latest` bundle receive fixes.

## In scope

- Pairing and transport: PAKE pairing, channel binding, session resume, QUIC
  framing (`carrova-crypto`, `carrova-transport`).
- Archive encryption and key derivation.
- Recipe bundle signing chain and client-side verification (`carrova-bundle`,
  `keys/bundle-roots.json`), including install and rollback of bundles.
- Receiver-side safety: path confinement, symlink escapes, permission restore,
  writes before plan approval, apply/rollback integrity.
- Secrets or file contents leaking into logs, error reports or events.
- Desktop IPC surface (`apps/desktop/src-tauri`) and the app updater.
- Release workflows and supply chain (`.github/`).

## Out of scope

- Attacks requiring an already compromised sender or receiver account.
- A recipe that moves data the user explicitly selected and confirmed.
- Denial of service on the local network by a host that can already drop or
  block LAN traffic.
- Vulnerabilities in third-party dependencies without a Carrova-specific impact
  (report them upstream; we track advisories via Dependabot).
