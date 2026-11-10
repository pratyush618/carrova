# Packaging Carrova

Carrova ships for Ubuntu as an **AppImage** and a **.deb**, both built by the
Tauri bundler from `apps/desktop`. The CLI installs with Cargo.

| Piece | Where |
|---|---|
| Bundle config (icons, .deb metadata, updater) | `apps/desktop/src-tauri/tauri.conf.json` |
| Release-only overlay (embeds the recipe bundle) | `apps/desktop/src-tauri/tauri.release.conf.json` |
| Desktop entry template (`Categories=Utility;System;`) | `packaging/linux/carrova.desktop` |
| CI release | `.github/workflows/release.yml` (`v*` tags) |

The recipe bundle is embedded only by the release overlay because
`tauri-build` checks resource paths on every build: debug builds, `clippy`
and tests must not need `dist/recipes`.

> **Open follow-up (release.yml):** the app job must also run `just bundle N`
> + `just bundle-sign` (needs `BUNDLE_SIGNER_KEY`/`BUNDLE_SIGNER_CERT`) and pass
> `args: --config src-tauri/tauri.release.conf.json` to tauri-action, or the
> released app ships without its recipe bundle.

## Build locally

Prerequisites: `just setup-ubuntu`, `just ui-install`, the dev keys in
`~/.config/carrova-dev/` (see below). A release build compiles ~500 crates:
check `uptime`/`df -h .` first and keep ≥ 15 GiB free.

```sh
just package            # bundle recipes → sign → AppImage + .deb
just package 1 deb      # .deb only (no network needed)
```

`just package` runs `just bundle N` + `just bundle-sign` (writes the signed
bundle to `dist/recipes`, mapped to `recipes/` inside the app) and then
`just tauri build --config src-tauri/tauri.release.conf.json`. Output:
`target/release/bundle/{deb,appimage}/`. AppImage bundling downloads
`linuxdeploy` on first use (network).

Because `bundle.createUpdaterArtifacts` is on, every `tauri build` needs the
updater private key in the environment (`just package` reads the dev key by
default):

```sh
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.config/carrova-dev/updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.config/carrova-dev/updater.key.password)"
```

The CLI: `cargo install --path apps/cli` (binary `carrova`).

## Keys

Two independent key systems; neither private key is ever committed.

| Key | Signs | Dev copy | Public half lives in |
|---|---|---|---|
| Updater (minisign, Tauri) | app updates (`latest.json`, `.sig`) | `~/.config/carrova-dev/updater.key` + `updater.key.password` (0600) | `tauri.conf.json` → `plugins.updater.pubkey` |
| Recipe bundle root (Ed25519, offline) | short-lived signer certificates | `~/.config/carrova-dev/bundle-root.key` | `keys/bundle-roots.json` |
| Recipe bundle signer (Ed25519, CI) | each recipe bundle | `~/.config/carrova-dev/bundle-signer.key` + `.cert.json` | certified by the root |

Generate a fresh updater key pair (password from a 0600 file, no prompt):

```sh
umask 077
head -c 32 /dev/urandom | base64 | tr -d '/+=' > updater.key.password
pnpm --filter desktop tauri signer generate --ci \
  -p "$(cat updater.key.password)" -w updater.key
```

Bundle keys: see `keys/README.md` (`cargo xtask bundle keygen|certify`).

## Before the first public release (user actions)

1. **Repository slug** is `pratyush618/carrova` in:
   - `apps/desktop/src-tauri/tauri.conf.json` (`plugins.updater.endpoints`, `bundle.homepage`)
   - `crates/carrova-report/src/issue.rs` (`ISSUE_REPOSITORY`, where "Send report" opens issues)
   - `crates/carrova-bundle/src/fetch/http.rs` (`DEFAULT_BASE_URL`, recipe bundle releases)
   - `apps/cli/Cargo.toml` (`repository`)

   If the repository moves, update all four; `just release-preflight` rejects
   the `OWNER/REPO` placeholder.
2. **Replace the DEV updater key**: generate a maintainer key pair offline,
   put its public key in `plugins.updater.pubkey`, store the private key and
   password as the `TAURI_SIGNING_PRIVATE_KEY` /
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secrets. Losing this key means
   existing installs can never auto-update again — back it up offline.
3. **Replace the DEV bundle root** in `keys/bundle-roots.json` with the
   offline root key; certify a CI signer (≤ 90 days) and store it as
   `BUNDLE_SIGNER_KEY` / `BUNDLE_SIGNER_CERT`.
4. Replace the maintainer placeholder (`authors` in
   `apps/desktop/src-tauri/Cargo.toml` → `.deb` `Maintainer:`).

## GitHub secrets (environment `release`)

| Secret | Used by |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | `release.yml` app job (tauri-action signs updater artifacts) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | same |
| `BUNDLE_SIGNER_KEY` | `release.yml` recipes job (`just bundle-sign`) |
| `BUNDLE_SIGNER_CERT` | same |

`GITHUB_TOKEN` is provided by Actions. The environment should require a
reviewer and allow only `v*` / `recipes-v*` tags.

## Exit-gate smoke test (clean Ubuntu 24.04 VM)

1. Fresh VM, fully updated, no dev tools. Copy the `.deb` and AppImage.
2. `.deb`: `sudo apt install ./Carrova_<ver>_amd64.deb` — dependencies
   resolve from the archive; `dpkg -s carrova` shows the maintainer and
   section `utils`.
3. Launch from the app grid (entry under Utilities/System, icon shown) and
   from a terminal (`carrova-desktop`): the welcome screen asks for
   error-report consent; nothing is in `~/.local/state/carrova/reports/`
   until you allow it.
4. Recipes: the catalogue lists the 15 bundled recipes with no network
   (`/usr/lib/Carrova/recipes/index.json` exists).
5. Run a wizard migration from a second machine (LAN) and from an archive
   file; undo it from History.
6. Updates: publish a higher draft version, then **Check for updates** —
   the version and notes are shown, nothing installs until you confirm;
   after install, **Restart** runs the new version.
7. Recipe update: publish `recipes-v<N+1>` with a new recipe; the app shows
   it without an app release.
8. AppImage: `chmod +x Carrova_<ver>_amd64.AppImage && ./Carrova_<ver>_amd64.AppImage`
   repeats steps 3–6 (needs `libfuse2t64` on 24.04).
9. Uninstall: `sudo apt remove carrova` leaves user data in `~/.local/state/carrova`.
