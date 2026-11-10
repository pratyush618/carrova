---
name: new-recipe
description: Use when adding or changing a Carrova app recipe (recipes/<category>/<app>/recipe.yaml + fixture). Covers schema, flags, fixture test, validation.
---

# New recipe

A recipe is declarative data describing one app: how to detect it and which items (files/dirs) can move. No code, no absolute paths.

## Steps

1. Pick category dir under `recipes/` (e.g. `dev-tools`, `shells`, `editors`). App id = lowercase kebab-case, unique.
2. Create `recipes/<category>/<app>/recipe.yaml`.
3. Create `recipes/<category>/<app>/fixture/` — fake home dir tree with representative files (dummy contents, never real secrets).
4. Validate: `cargo run -p carrova-cli -- recipes validate recipes/<category>/<app>`.
5. Fixture test: `just fixtures <app>`.
6. Run `just ci` before calling it done.

## Shape (`recipes/schema/recipe.v<N>.json` is authoritative)

Use `schema: 1` unless the recipe needs a v2 field. Example (v1):

```yaml
schema: 1
id: git
name: Git
category: dev-tools
detect:
  linux:
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
      portability: same_os     # same_os | cross_os
      sensitive: false
      requires_closed: false
      default: true
```

## Schema v2 additions

v2 = v1 plus two optional fields. Using either with `schema: 1` is a validation error.

```yaml
schema: 2
id: gnupg
# ...
processes: [gpg-agent, scdaemon, dirmngr, keyboxd]   # recipe-level
items:
  - id: public-keyring
    label: Public keyring and trust database
    handler: gpg-keyring
    paths:                                            # files the handler mutates
      - { root: home, rel: .gnupg/pubring.kbx }
      - { root: home, rel: .gnupg/trustdb.gpg }
```

- `processes`: extra process names (daemons/helpers) that count as "app running" for `requires_closed` items, besides `binary` detect rules. Bare names, no `/`.
- `handler`: name of a built-in Rust handler (closed set, `carrova_recipe::Handler`; currently `gpg-keyring`). Unknown name = load error. Capture/apply go through the handler, not raw files. `paths` stay required and declare what the handler mutates on the receiver (dry-run plan + rollback backup). Not allowed on unmovable items.
- Adding a new handler = Rust change in `carrova-recipe` (`Handler` enum) + `recipe.v2.json` `$defs.handler` enum + implementation in `carrova-core`. Recipes never carry commands.
- Fixture `expected.yaml`: handler items list `handler: <name>` and their declared paths under `files`; optional top-level `process_names` asserts `Recipe::process_names(Linux)`.

## Rules

- Roots only: `user_config`, `user_data`, `cache`, `home`, `documents`. Relative paths must not contain `..` or start with `/`.
- Secrets (SSH/GPG keys, tokens, credential stores) → `sensitive: true`, `default: false`.
- Caches, logs, lock files → `default: false` or excluded.
- Databases or files with locks → `requires_closed: true`.
- Prefer a built-in `handler` (schema v2) over raw files when the app owns the format (e.g. GPG public keyring → `gpg-keyring`). No handler yet? Use raw files and note it; never put commands in a recipe.
- OS-bound secrets (browser passwords, keyrings) are not portable — mark as an unmovable item with a reason, don't capture.
- Daemons that keep files open (agents, helpers) → list in `processes` (schema v2).
- v1 scope (Ubuntu → Ubuntu): everything is `same_os`.
