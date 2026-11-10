# Plugins

Plugins give recipes logic that data cannot express, e.g. rewriting the
absolute paths an app stores in its config. They are WebAssembly modules run
by `carrova-plugin` in a sandbox: no imports at all (no files, network,
processes, clock), per-file fuel and memory limits, and a fresh instance for
every file. The ABI, limits and threat model are documented in
`crates/carrova-plugin/src/lib.rs`.

v1 plugins are **receiver-side content transforms**: for each file of a
recipe item with `handler: { plugin: <id> }` (recipe schema 3), Carrova
passes the file's bytes, its path and the context fields the manifest
declares to `transform`, then stages, verifies and applies the output like
any other file. The user allows each plugin per import after seeing its
permissions (`--allow-plugin ID` in the CLI).

## Layout

```
plugins/<id>/
  plugin.yaml   manifest (schema 1): id, version, description, context, limits
  plugin.wasm   built module (`just plugins`), committed
  Cargo.toml    standalone crate (not in the workspace), wasm32-unknown-unknown
  src/, tests/  source; keep the logic in safe Rust testable on the host
```

`plugin.yaml`:

```yaml
schema: 1
id: path-rewrite                  # what recipes name
version: 1.0.0
description: One sentence the user sees before allowing it.
context: [source_home, target_home, source_os, target_os]   # only what it needs
limits: { fuel: 500000000, memory_mib: 64 }                 # per file, fuel ≤ 1e9
```

## Authoring

1. Copy `path-rewrite/` and rename the crate and `id` (`[a-z0-9-]`, ≤ 64).
2. Export `memory`, `alloc` and `transform` (see the ABI); import nothing.
3. Declare only the context fields you read; each one is a permission line.
4. `just plugins`: host tests + release build to `plugin.wasm` (the wasm
   target comes from `rust-toolchain.toml`).
5. Name it from a recipe item: `handler: { plugin: <id> }`.

## Review and signing

- Plugins are **core tier only** (`.github/CODEOWNERS`: `/plugins/` →
  core maintainers). There is no community tier for code.
- Review the source; `just plugins-check` (CI and the release gate) rebuilds
  every `plugin.wasm` and fails if the committed module differs.
- They ship only inside the signed recipe bundle (`cargo xtask bundle build`
  picks up `plugins/` next to `recipes/`), as content-addressed blobs listed
  in the signed index. The `--recipes DIR` developer override loads unsigned
  plugins from `DIR/../plugins`, with the usual unverified warning.
