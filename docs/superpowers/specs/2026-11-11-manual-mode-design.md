# Manual mode and "Send with Carrova" — design

Date: 2026-11-11 · Status: awaiting user approval

## Why

Today a send starts from a scan: Carrova finds the apps it knows and lists big
home folders it doesn't recognise. A user who just wants "these three things"
(a project folder, one `.env`, a notes file) can't point at them. They also
can't start a send from where they already are: the file manager.

## What we're building

1. **Manual switch on the Send screen.** Off = **Auto** (today's flow: found
   apps, suggestions, unrecognised folders). On = **Manual**: the user browses
   with the system file picker and picks exact files and folders. One mode at
   a time: what is sent is the list of the mode that is showing.
2. **Setting "Start sends in Manual mode"** (Settings → Sending, default off).
   Send opens in that mode; the switch still works on the screen.
3. **"Send with Carrova" in the file manager's right-click menu** (Linux,
   any distro and desktop).
   Right-click files/folders → the app opens (or comes to front) on the Send
   screen in Manual mode with those items already picked.
4. **CLI parity:** `carrova send PATH...` and `carrova export PATH...` pick
   paths the same way (Manual mode in the CLI).

Mockups: brainstorm session screens `modes.html` (layout B2 = the Manual list)
and `toggle.html` (option 1 + 2 + right-click menu).

## Goals

- Pick any file or folder inside the user's semantic roots (home, config,
  data, cache, documents), at any depth.
- Same safety as Auto: dry-run review on the receiver, staged apply, undo,
  sensitive items off until confirmed; picks arrive unticked on the
  receiver with an "Accept all" for safe ones.
- Same behaviour from the CLI and the desktop app.
- Archive ("Save to a file") works in Manual mode too — same selection.

## Non-goals

- Paths outside the user's semantic roots (`/etc`, other disks, `/mnt`).
- Mixing modes in one move (apps *and* picks). Switching modes keeps both
  lists in memory but sends only the visible one.
- Right-click integration on Windows/macOS (Phase 5 platform work, later).
- Flatpak right-click integration (deferred with Flathub).

## Anti-goals (failure even if it "works")

- A pick that bypasses the sensitive-key confirmation or the dry-run.
- A pick that restores outside the receiver's semantic roots, or follows a
  symlink out of them.
- OS-specific code in `carrova-core` or business logic in `src-tauri`.
- Manual mode feeling like a different app: it uses the same screens,
  components and pairing flow.

## Decisions

### Mode behaviour (my calls)
- Switching Auto ↔ Manual keeps both lists; the footer and "Next" act on the
  visible mode only. The switch label shows the other mode's count
  ("Manual · 2").
- Manual start skips the app scan; the scan runs the first time the user
  switches to Auto (keeps a right-click send instant).
- Two buttons, **Add files** and **Add folder** (GTK/portal pickers can't
  mix files and folders in one dialog). Multi-select allowed. Picker starts
  in home.
- Each pick row: icon, display path (`~/…`), size and file count, remove
  button. Sensitive picks show the amber key row and need the existing
  per-key confirm.
- A pick inside an already-picked folder is absorbed (not listed twice);
  picking a parent replaces its picked children. Duplicates are ignored.

### Engine (core owns the policy)
- New core API to add a user path to a selection: input = absolute path,
  output = an `ItemKey::raw(RootedPath)` + size/file count, or a typed
  rejection. The **most specific** semantic root containing the path is
  used (e.g. `~/.config/foo` → `user_config/foo`, not `home/.config/foo`),
  so receiver root checks apply as for any item (cross-OS: same-OS only,
  see User decisions).
- Rejections (typed, shown in plain words, CLI prints the same): outside
  every semantic root; the pick itself is a symlink pointing outside a
  root; unreadable; is a semantic root itself (e.g. the whole home folder —
  my call: refuse, too big and would sweep caches; user picks folders
  inside it instead).
- Root resolution goes through `Platform` (root dirs + normalisation); no OS
  code in core.
- Raw items today are top-level folders from the scan. Picks extend raw
  items to **any depth** and to **single files**. Receiver planning,
  manifest and restore must accept these (verify; extend if the receiver
  assumes raw = top-level dir).
- **Sensitive detection for picks** (my call): a policy table in core marks
  a pick sensitive if it is or contains well-known secret locations/names
  (`.ssh/`, `.gnupg/`, `id_*` private keys, `*.pem`, `*.key`, `.env`,
  `.netrc`, `.pgpass`, cloud credential dirs such as `.aws/`, `.kube/`).
  Sensitive picks default off and need explicit confirmation, like recipe
  keys. Permissions are restored exactly.
- Symlinks inside a picked folder: never followed, same rules as today.

### Desktop
- The Send flow model gains a mode (`auto | manual`), the picks list and the
  intents add-files / add-folder / remove / switch-mode. View-model logic
  stays in `flows/send`, screens stay dumb.
- Native picker through the existing dialog host (`engine/hosts.ts`,
  `@tauri-apps/plugin-dialog`), with `multiple: true` and `directory` for
  Add folder. Paths go to the engine through a typed IPC command; the
  engine answers with the pick or the rejection reason.
- Setting persisted with the other device settings (`SettingsStore`).
- i18n strings in `i18n/en/send.ts` + `settings.ts`; gallery entries for the
  new states (empty Manual list, picks, rejected pick, sensitive pick).

### Right-click entry (Linux — any distro, any desktop; user 2026-11-11)
There is no single Linux standard for file-manager menus, so "works
everywhere" = a native entry for each common file manager **plus** a
universal fallback:

| File manager (desktops) | Integration | Extra package needed |
|---|---|---|
| Nautilus (GNOME, Ubuntu, Fedora, Pop) | `nautilus-python` extension → "Send with Carrova" in the main menu | `nautilus-python` (name varies by distro) |
| Nautilus without that package | Nautilus script → "Scripts → Send with Carrova" | none |
| Nemo (Cinnamon, Mint) | `.nemo_action` file | none |
| Dolphin (KDE) | KIO service menu `.desktop` | none |
| Caja (MATE), PCManFM-Qt (LXQt) | freedesktop file-manager actions `.desktop` (`file-manager/actions`) | none (Caja: `caja-actions`) |
| Thunar (XFCE) | custom action merged into the user's `uca.xml` | none |
| Anything else | nothing — "Open With" entry removed (2026-11-11): a per-user `MimeType=inode/directory` entry can become the default folder opener where no `mimeapps.list` default exists | — |

- **Installed per user, from the app**, so it works for every package
  format (.deb, .rpm, AppImage, tarball) and every distro: the files go to
  the user's data/config dirs (`~/.local/share/nautilus-python/extensions`,
  `~/.local/share/nautilus/scripts`, `~/.local/share/nemo/actions`,
  `~/.local/share/kio/servicemenus`, `~/.local/share/file-manager/actions`,
  `~/.config/Thunar/uca.xml`). The integration points at the running
  binary's path (AppImage: `$APPIMAGE`), refreshed on each start if moved.
- This makes the right-click entry a **Settings toggle** after all:
  Settings → Sending → "Show 'Send with Carrova' in the file manager"
  (off until the user accepts the one-time Home card, see User decisions;
  turning off removes every file it wrote, and only those). Thunar's `uca.xml` is edited by adding/removing only our
  own action (matched by a unique id), never touching the user's others.
- Writing these files is OS-specific → a `carrova-platform` trait method
  (Linux adapter implements; Windows/macOS return `Unsupported` for now).
  Core/front-ends only call it.
- Packages (.deb/.rpm) **Recommend** the distro's `nautilus-python`
  package, so Ubuntu/Fedora users get the top-level menu item; without it
  Nautilus users still get the Scripts entry.
- Flatpak: deferred with Flathub (sandbox can't write these dirs).
- The app accepts `--send PATH...`. A single-instance plugin
  (`tauri-plugin-single-instance`) forwards the paths to a running window,
  which adds them to the Manual list and comes to the front.
- If a move is already in progress, forwarded paths are not added; the app
  shows a plain message ("Finish or cancel the current move first").

### CLI
- `carrova send [PATH...]` and `carrova export [PATH...]`: paths given →
  Manual mode (no app scan), each path resolved by the same core API,
  rejections printed in the same words. Combining paths with `--select`/
  presets is an error (one mode at a time).

### User decisions (2026-11-11)
- **Receiver: picks arrive unticked + "Accept all".** Today's rule stays:
  data with no recipe arrives unticked on the receiver
  (`plan/planner/sensitivity.rs`, `plan/options.rs`). New: the review shows
  one "Accept all N files" action for the picked group. It ticks picks that
  are neither sensitive (sender flag **or** receiver-side match against the
  same sensitive table) nor in `DANGEROUS_PATHS` (e.g. `~/.bashrc`,
  autostart). Those still need a tick each. The receiver trust rule is not
  relaxed: the archive flag can still only add sensitivity.
- **Cross-OS: picks are same-OS only.** They keep today's raw-item
  behaviour (`SameOs`, blocked across OSes, per-item override restores to
  the same place relative to the root). No new mapping. Fix the doc claim:
  picks do *not* map across OSes like recipe items.
- **Right-click: ask once.** A one-time dismissible card on Home ("Add
  'Send with Carrova' to your file manager?"), shown to new installs and
  to upgrades. Nothing is written until the user says yes. Settings toggle
  afterwards (replaces "default on at first run" above).

### All platforms (user 2026-11-11: "I want for all platforms, and the next release should contain these also")
Supersedes "Linux only" and the Windows/macOS non-goal. Same trait method,
same Home card + Settings toggle + CLI `integrate`, per-user, no admin:
- **Windows:** per-user registry verbs under `HKCU\Software\Classes\*\shell\Carrova`
  and `HKCU\Software\Classes\Directory\shell\Carrova` ("Send with Carrova",
  `"<exe>" --send "%1"`; Windows 11 shows it under "Show more options"),
  plus a "Carrova" shortcut in the user's `SendTo` folder (multi-select works
  there). Registry via a safe crate (`winreg`), no `unsafe`. Remove deletes
  only our keys and shortcut.
- **macOS:** a Finder Quick Action (`~/Library/Services/Send with
  Carrova.workflow`, Run Shell Script → `open -a Carrova --args --send "$@"`),
  shown under right-click → Quick Actions / Services. No Finder Sync
  extension (needs a signed app extension). The app handles `--send` args
  only; no macOS "open documents" handler (`open -n` starts a fresh
  instance, which forwards its argv like any second launch).
- **Late calls (main thread, 2026-11-11):** Windows drops the `SendTo`
  shortcut (the only `.lnk` crate, `mslnk`, is unmaintained and panics
  internally); the two HKCU verbs cover files and folders, multi-select
  launches once per file and the running app collects them. macOS Quick
  Action opens the exact `.app` that holds the launcher
  (`open -n -a '<bundle>' --args --send "$@"`; `-n` so a running app receives the paths via single-instance forwarding), refusing builds outside a bundle.
- **Release:** the next release ships this feature; whether it also ships
  macOS/Windows builds is decided separately (release jobs for them are
  commented out until the Phase 5 gate).
- Verification limits: no Windows box (CI `windows-latest` + cross-check);
  macOS on the user's M1.

### Builder-check calls (main thread, 2026-11-11)
- **Picks that contain another semantic root are refused** (e.g. `~/.local`
  holds `user_data`), same rule the unrecognised scan already applies
  (`scan/unrecognised.rs`). The message tells the user to pick inside it.
- **Sender-side sensitive flag:** picks matched by the sensitive table set
  the manifest `sensitive` flag (today raw items hard-code
  `ItemFlags::default()` in `capture/items.rs`).
- **Scan-less session start:** Manual needs a send session that starts
  without a scan, a `Selection` that accepts added picks, and a later Auto
  scan merged into the same session (today `Exporter::start_with` scans
  first, `Selection` is built only by `from_scan`, edits on unknown keys are
  refused). Builder designs the API.
- **CLI:** paths are positional. `--confirm-sensitive` accepts a path as
  well as an item key; without it, an interactive confirm is asked per
  sensitive pick (`--yes` does not confirm sensitive items). Legal with
  paths: `--exclude-path`, `--confirm-sensitive`, `--force-unmovable`.
  Illegal: an explicit `--preset`, `--include`, `--exclude` (detect
  explicit flags; `--preset` has a default).
- **CLI parity for the right-click setting:**
  `carrova integrate file-manager on|off|status`.
- **Stale entries after uninstall:** every integration launches through
  the recorded binary path; if it's gone, the entry shows nothing / exits
  silently and the next app start rewrites or removes it. Package removal
  scripts don't touch home dirs.

## Constraints

- Boundary rules in `CLAUDE.md`: OS code only in `carrova-platform`;
  `src-tauri` thin; dependencies point down.
- No change to `carrova-crypto` or the transport handshake.
- Tests outside `src/` per the layout rule; bug fixes test-first.
- Two-engine core test: manual pick (deep folder + single file + sensitive
  file) sent and restored, plus rejections. CLI golden for pick rejections.
  Vitest for the flow model; Playwright for the Manual switch and picker
  happy path (fake dialog host).
- `just ci` green, plus `just e2e`, `just e2e-ui`.

## Left to the builder

- Exact names of the core API, IPC command and view-model types.
- Whether absorbing nested picks lives in core (preferred, one policy) or
  the flow model.
- Icons, exact copy wording within the existing voice.
- How the Nautilus extension finds the binary (desktop file id vs PATH).
