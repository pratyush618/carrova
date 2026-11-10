# Packaging Carrova

Carrova ships for Ubuntu as an **AppImage** and a **.deb**, for macOS as a
universal **.dmg** (Apple Silicon + Intel, macOS 11+), and for Windows as an
**.msi** and an NSIS **.exe** installer, all built by the Tauri bundler from
`apps/desktop`. Flathub repackages the `.deb`. The CLI installs with Cargo.

| Piece | Where |
|---|---|
| Bundle config (icons, .deb metadata, updater) | `apps/desktop/src-tauri/tauri.conf.json` |
| Per-OS overlays (targets, macOS min version, Windows signing digest) | `tauri.macos.conf.json`, `tauri.windows.conf.json` (merged by Tauri automatically) |
| Icons (`.icns`/`.ico` generated from `icons/icon.svg`) | `apps/desktop/src-tauri/icons/` |
| Flathub manifest + AppStream metainfo | `packaging/flatpak/` |
| Release-only overlay (embeds the recipe bundle) | `apps/desktop/src-tauri/tauri.release.conf.json` |
| Desktop entry template (`Categories=Utility;System;`) | `packaging/linux/carrova.desktop` |
| CI release | `.github/workflows/release.yml` (`v*` tags) |

The recipe bundle is embedded only by the release overlay because
`tauri-build` checks resource paths on every build: debug builds, `clippy`
and tests must not need `dist/recipes`.

## Build locally

Prerequisites: `just setup-ubuntu`, `just ui-install`, the signer and updater
keys in `~/.config/carrova-release/` (see below; the root key is not needed). A release build compiles ~500 crates:
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
updater private key in the environment (`just package` reads the local release key by
default):

```sh
export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.config/carrova-release/updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.config/carrova-release/updater.key.password)"
```

The CLI: `cargo install --path apps/cli` (binary `carrova`).

### macOS and Windows

Build on the target OS (Tauri does not cross-bundle). No extra system
packages: macOS needs Xcode command-line tools, Windows the MSVC build tools
and WebView2 (preinstalled on Windows 10/11).

```sh
just package 1 app,dmg     # macOS, host arch (release CI is Linux-only for v0.1)
just package 1 msi,nsis    # Windows, from Git Bash
```

Icons: regenerate `icon.icns`/`icon.ico` after changing `icons/icon.svg` with
`pnpm --filter desktop tauri icon src-tauri/icons/icon.svg -o /tmp/icons` and
copy those two files back (the PNGs are already in place; Android/iOS/Store
outputs are not used).

From Linux, `just check-targets` cross-`cargo check`s the crates that have no
C build dependencies (`carrova-platform`, `-recipe`, `-crypto`, `-plugin`,
`-report`) for `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`. Everything
that pulls in `ring` or `blake3` needs the target's C toolchain and is covered
only by the macOS/Windows jobs in `engine.yml` (`just ci-portable`).

## Keys

Two independent key systems; neither private key is ever committed.

| Key | Signs | Local copy | Public half lives in |
|---|---|---|---|
| Updater (minisign, Tauri) | app updates (`latest.json`, `.sig`) | `~/.config/carrova-release/updater.key` + `updater.key.password` (0600) | `tauri.conf.json` → `plugins.updater.pubkey` |
| Recipe bundle root (Ed25519, offline) | short-lived signer certificates | offline only, never on a packaging host (needed only for `bundle certify`) | `keys/bundle-roots.json` |
| Recipe bundle signer (Ed25519, CI) | each recipe bundle | `~/.config/carrova-release/bundle-signer.key` + `.cert.json` | certified by the root |

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

### OS code signing (optional secrets, same `release` environment)

Each block below is skipped when its secrets are absent: the release still
builds, but the packages are unsigned (macOS Gatekeeper blocks a
non-notarized download until the user right-clicks → Open; Windows
SmartScreen warns). The updater signature (`TAURI_SIGNING_*`) is separate and
always required.

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the exported **Developer ID Application** `.p12` (`base64 -i cert.p12 \| pbcopy`, one line) |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` export password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Jane Doe (TEAMID1234)` (`security find-identity -v -p codesigning`) |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | notarization via Apple ID: account email, an [app-specific password](https://support.apple.com/102654), team ID |
| `APPLE_API_ISSUER` / `APPLE_API_KEY` / `APPLE_API_PRIVATE_KEY` | *or* notarization via an App Store Connect API key (Users and Access → Integrations): issuer ID, key ID, contents of `AuthKey_<id>.p8`. Used instead of the Apple ID trio when `APPLE_API_KEY` is set |
| `WINDOWS_CERTIFICATE` | base64 of the code-signing `.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD` | the `.pfx` password |

What the maintainer must provide for macOS: a paid **Apple Developer Program**
membership, a *Developer ID Application* certificate (Xcode → Settings →
Accounts → Manage Certificates, or developer.apple.com) exported with its
private key as `.p12`, and the team ID. Tauri signs with the hardened runtime
and notarizes + staples the `.dmg`/`.app` during `tauri build` when the
certificate and one notarization variant are set.

Windows: the workflow imports the `.pfx` into the runner's certificate store
and passes its thumbprint to Tauri (`tauri.windows.conf.json` sets SHA-256 +
the DigiCert timestamp server). This works for OV certificates exported as
`.pfx`; EV and post-2023 OV certificates live on hardware tokens or cloud
HSMs (e.g. Azure Artifact Signing) and need `bundle.windows.signCommand`
instead — see <https://v2.tauri.app/distribute/sign/windows/>.

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

macOS / Windows smoke (same release draft): install the `.dmg` on the M1
(opens without a Gatekeeper warning when notarized; `spctl -a -vv
/Applications/Carrova.app` says `Notarized Developer ID`) and the `.msi` or
`-setup.exe` on Windows, then repeat steps 3–6. `latest.json` on the draft
must list `linux-x86_64`, `darwin-aarch64`, `darwin-x86_64` and
`windows-x86_64`.

## Flathub

`packaging/flatpak/dev.carrova.app.yml` repackages the release `.deb`
(Tauri's documented route) on the GNOME runtime, which provides
WebKitGTK 4.1. `dev.carrova.app.metainfo.xml` is the AppStream file; the
desktop entry is the one Tauri renders from `packaging/linux/carrova.desktop`
into the `.deb`, renamed to the app id.

Sandbox caveats (expect Flathub reviewers to ask): Carrova needs
`--filesystem=home` and the host XDG paths (the manifest unsets the
sandbox's `XDG_*_HOME`), and inside the sandbox it cannot see host processes,
so "close the app first" checks cannot detect running apps. The `.deb` and
AppImage have none of these limits.

Local check:

```sh
appstreamcli validate packaging/flatpak/dev.carrova.app.metainfo.xml
flatpak install flathub org.gnome.Platform//49 org.gnome.Sdk//49
flatpak-builder --user --install --force-clean build-dir packaging/flatpak/dev.carrova.app.yml
flatpak run dev.carrova.app
```

Submission (user):

1. Publish a `v*` release; set `url` + `sha256` (`sha256sum Carrova_<ver>_amd64.deb`)
   in the manifest and add a `<release>` entry plus `<screenshots>` to the
   metainfo. Bump `runtime-version` to the current GNOME runtime.
2. Verify the domain `carrova.dev` (the app id `dev.carrova.app` implies it)
   or switch to `io.github.pratyush618.carrova` (app id, file names, Tauri
   `identifier`) if you don't own it.
3. Fork <https://github.com/flathub/flathub>, branch from `new-pr`, add the
   manifest + metainfo, open a PR against `new-pr`, and follow review.
4. After merge, Flathub creates `flathub/dev.carrova.app`; each release is a
   PR there bumping `url`/`sha256` (or enable `x-checker-data` later).
