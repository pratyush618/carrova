# Contributing a recipe

A recipe is declarative data describing one app: how to detect it and which
items (files and folders) can move to another machine. **No code, no absolute
paths, no commands.** Recipes ship to users as a signed bundle, independently of
app releases, so a merged recipe reaches everyone without a new Carrova version.

General setup and the PR process are in the root [`CONTRIBUTING.md`](../CONTRIBUTING.md).

## Steps

1. Pick a category directory (`dev-tools`, `editors`, `security`, `shells`,
   `terminals`, …).
   The app id is lowercase kebab-case, unique, and equals the directory name.
2. Write `recipes/<category>/<app>/recipe.yaml`.
3. Build a fixture: `recipes/<category>/<app>/fixture/home/` is a fake home
   directory with representative files (**dummy contents, never real secrets**),
   and `fixture/expected.yaml` lists what must and must not be captured.
   Add `fixture.macos/` / `fixture.windows/` where the files sit elsewhere on
   that OS (see [Fixture](#fixture)).
4. Validate: `cargo run -p carrova-cli -- recipes validate recipes/<category>/<app>`
5. Fixture test: `just fixtures <app>`
6. `just ci` before opening the PR.

## Schema

`recipes/schema/recipe.v<N>.json` is authoritative. Use the lowest schema
that has the fields you need: `schema: 1`, `2` for `processes`/`handler`, `3`
for per-OS `os` overrides, plugin handlers or a recipe without Linux rules.

```yaml
schema: 1
id: git
name: Git
category: dev-tools
detect:
  linux:                       # detected if ANY rule matches
    - binary: git
    - path: { root: home, rel: .gitconfig }
items:
  - id: config
    label: Global git config
    paths:
      - { root: home, rel: .gitconfig }
      - { root: user_config, rel: git }
    exclude: ["**/credentials"]
    flags:
      portability: same_os     # or cross_os (see "Other OSes")
      sensitive: false
      requires_closed: false
      default: true
```

**Schema v2** adds two optional fields (using them with `schema: 1` is an error):

- `processes: [gpg-agent, …]` (recipe level): daemons/helpers that count as
  "app running" for `requires_closed` items.
- `handler: <name>` (item level): a built-in Rust handler from a closed set
  (currently `gpg-keyring`) that captures/applies the item instead of raw file
  copy. `paths` still declare what it touches on the receiver. A new handler is
  an engine change, reviewed separately, never a recipe-only change.

**Schema v3** adds per-OS support (next section).

### Other OSes (schema v3)

A recipe applies on an OS only if it has `detect.<os>` rules (`linux`,
`macos`, `windows`; at least one). Semantic roots resolve per OS:

| Root | Linux | macOS | Windows |
|---|---|---|---|
| `user_config` | `~/.config` | `~/Library/Application Support` | `%APPDATA%` |
| `user_data` | `~/.local/share` | `~/Library/Application Support` | `%LOCALAPPDATA%` |
| `cache` | `~/.cache` | `~/Library/Caches` | `%LOCALAPPDATA%` |
| `home` | `~` | `~` | `%USERPROFILE%` |
| `documents` | `~/Documents` | `~/Documents` | Documents known folder |

When the app does not follow that convention (git, nvim and most CLI tools
read `~/.config` on macOS), give the item an `os` override with the real
location. Check the app's own docs or source; don't guess.

```yaml
  - id: config
    paths:
      - { root: home, rel: .gitconfig }        # slot 0
      - { root: user_config, rel: git }        # slot 1
    os:
      macos:
        paths:
          - { root: home, rel: .gitconfig }    # slot 0, same data
          - { root: home, rel: .config/git }   # slot 1, same data
        # exclude: [...]   optional; replaces the base exclude ([] = none)
```

- **Slots line up.** `os.<os>.paths[i]` is where the data of base
  `paths[i]` lives on that OS; both lists have the same length (the loader
  checks). Cross-OS restore maps files slot by slot, so never reorder.
- An OS without an override uses the base `paths` and `exclude`. A recipe
  whose paths are all under `home` (ssh, cargo, npm) usually needs none.
- A recipe for one OS only (Windows Terminal) puts that OS's paths in the
  base `paths` and has only that OS's `detect` rules.
- **Windows separators.** Inside an `os.windows` block and in
  `detect.windows` path rules `rel` may use `\` (`rel: 'Code\User'`,
  normalised to `Code/User`). Base, `linux` and `macos` paths (items and
  detect rules) must use `/`: there `\` is a filename character
  and is rejected. Drive letters (`C:\…`), UNC (`\\server\…`) and any other
  absolute form are rejected everywhere: pick the semantic root instead
  (`root: user_config` for `%APPDATA%`).
- `binary` detect rules double as process names for `requires_closed`.
  On Windows a process name is the exe file stem with its exact case
  (`Code`, not `Code.exe` or `code`), and a `binary` rule is looked up on
  `PATH` with `PATHEXT` (`code` finds `code.cmd`). When the CLI shim differs
  from the running app, list the app in `processes` (schema 2+).

**Portability.** `portability: cross_os` means the item restores on another
OS, slot by slot. Use it only when the content is genuinely OS-neutral: the
app reads the same format everywhere and the files normally hold no absolute
paths or OS-specific settings (VS Code `settings.json`, starship.toml,
`.gitconfig`, `wezterm.lua`). Keep `same_os` for caches, databases, installed
plugins and binaries, files whose content is usually OS-specific (shell
startup files, `gpg-agent.conf` with a pinentry path) and anything whose
permissions matter (SSH private keys). The user can still force a `same_os`
item across OSes behind a warning.

### Paths: semantic roots only

Every path is a `(root, rel)` pair. Roots: `user_config`, `user_data`, `cache`,
`home`, `documents`. `rel` uses `/` (also `\` inside `os.windows`), never
starts with a separator, `~` or a drive letter, and never contains `..`.
The receiver re-checks confinement, so an escaping path is a
load error, not a silent skip.

### Flags and safe defaults

| Situation | Rule |
|---|---|
| Secrets: SSH/GPG keys, tokens, credential files, shell history | `sensitive: true`; starts unticked and needs explicit confirmation (`default: true` is rejected) |
| Caches, logs, lock files | `default: false`, or `exclude` them |
| Databases or files the app keeps open | `requires_closed: true` (+ `processes` for daemons) |
| OS-bound data: keyring logins, installed extensions/packages, native builds | an `unmovable: <reason>` item, so the user sees why it stays behind and what to do instead |
| App owns the format (e.g. GPG keyring) | prefer a built-in `handler` over raw files |

Write labels and `unmovable` reasons for users: plain words, one line, and tell
them what to do on the new machine ("Run `gh auth login` again").

### Fixture

`expected.yaml` asserts detection and the exact captured set:

```yaml
binaries: [fish]          # binaries the fake PATH provides
detected: true
items:
  config:
    files:
      - user_config:fish/config.fish
not_captured:             # excluded files that must NOT be captured
  - .config/fish/fishd.oldhost
```

Cover every item, every `exclude`, and at least one file per path. Fake keys
and tokens must be obviously fake (`dummy-…`, zero-filled).

`just fixtures` checks the recipe on every OS it has `detect` rules for, with
the fake home's roots laid out like that OS (`.config`; `Library/Application
Support`, `Library/Caches`; `AppData/Roaming`, `AppData/Local`; `Documents`).
It uses `fixture.macos/` / `fixture.windows/` when present, else `fixture/`.
So a recipe with only `home` paths reuses `fixture/`; one with `os`
overrides or `user_*` roots needs its own `fixture.<os>/` (same layout:
`home/` + `expected.yaml`, `process_names` checked for that OS). A
Windows-only recipe ships only `fixture.windows/`. A `fixture.<os>/` for an
OS the recipe has no rules for is an error.

## Trust tiers

- **community** (default): any merged recipe. Reviewed by `@carrova/recipe-reviewers`.
- **verified**: listed in [`TIERS.yaml`](TIERS.yaml) after a maintainer has
  checked it against real installs. Changes to a verified recipe are reviewed by
  `@carrova/recipes-verified`; promoting a recipe (editing `TIERS.yaml`) needs a
  core maintainer.

The desktop app shows the tier next to each app.

## Review checklist

Reviewers (and the `recipes` CI workflow) check that:

- [ ] `recipes validate` and `just fixtures <app>` pass; the bundle still builds.
- [ ] Only semantic roots + relative paths; no commands; no new handler.
- [ ] Every secret is `sensitive`; nothing machine-bound is copied.
- [ ] Caches and logs are excluded or off by default.
- [ ] Fixture contents are dummy data and cover every item and exclude.
- [ ] Labels and reasons are clear to a non-expert.

## Release

Maintainers publish recipes by tagging `recipes-vN`. CI builds the bundle,
signs it with the CI signing key (certified by the offline root key in
`keys/`), verifies it, and publishes it as the immutable `recipes-vN` GitHub
release. The rolling `recipes-latest` release holds only `latest.json`
(`{ "version": N }`); clients read it, then fetch every asset from
`recipes-vN`. Clients refuse a bundle more than 1000 versions ahead of the
installed one, so version numbers must grow by small steps.
