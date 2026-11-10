# Contributing a recipe

A recipe is declarative data describing one app: how to detect it and which
items (files and folders) can move to another machine. **No code, no absolute
paths, no commands.** Recipes ship to users as a signed bundle, independently of
app releases, so a merged recipe reaches everyone without a new Carrova version.

General setup and the PR process are in the root [`CONTRIBUTING.md`](../CONTRIBUTING.md).

## Steps

1. Pick a category directory (`dev-tools`, `editors`, `security`, `shells`, …).
   The app id is lowercase kebab-case, unique, and equals the directory name.
2. Write `recipes/<category>/<app>/recipe.yaml`.
3. Build a fixture: `recipes/<category>/<app>/fixture/home/` is a fake home
   directory with representative files (**dummy contents, never real secrets**),
   and `fixture/expected.yaml` lists what must and must not be captured.
4. Validate: `cargo run -p carrova-cli -- recipes validate recipes/<category>/<app>`
5. Fixture test: `just fixtures <app>`
6. `just ci` before opening the PR.

## Schema

`recipes/schema/recipe.v<N>.json` is authoritative. Use `schema: 1` unless you
need a v2 field.

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
      portability: same_os     # v1 (Ubuntu → Ubuntu): always same_os
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

### Paths: semantic roots only

Every path is a `(root, rel)` pair. Roots: `user_config`, `user_data`, `cache`,
`home`, `documents`. `rel` uses `/`, never starts with `/` or `~`, and never
contains `..`. The receiver re-checks confinement, so an escaping path is a
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
