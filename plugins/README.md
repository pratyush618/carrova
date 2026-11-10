# Plugins

Plugins give recipes logic that data cannot express, e.g. rewriting the
absolute paths an app stores in its config. They are WebAssembly modules run
by `carrova-plugin` in a sandbox: no files, network, processes or clock
unless the manifest declares a host import, per-file fuel and memory limits,
a per-run budget, and a fresh instance for every file. The ABI, limits and
threat model are documented in `crates/carrova-plugin/src/lib.rs`.

Plugins are **content transforms**: for each file of a recipe item with
`handler: { plugin: <id> }` (recipe schema 3), Carrova passes the file's
bytes, its path and the context fields the manifest declares to `transform`
on the receiver, then stages, verifies and applies the output like any other
file. With `export: true` the plugin's `export_transform` also runs on the
sender, before the file is sent. The user allows each plugin per import
after seeing its permissions (`--allow-plugin ID` in the CLI).

## Layout

```
plugins/<id>/
  plugin.yaml   manifest (schema 1): id, version, description, context, limits (+ run_tools, log, export)
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
# Optional, all off by default; each adds a consent sentence:
run_tools: [sqlite3]   # bare program names it may run via `carrova.run_tool`
log: true              # may call `carrova.log` (debug-level, capped)
export: true           # also exports `export_transform`, run on the sender
```

## Write a plugin

1. **Crate.** Copy `path-rewrite/` to `plugins/<id>/` and rename the crate and
   `id` (`[a-z0-9-]`, ≤ 64). It is a standalone `cdylib` (not a workspace
   member); keep the logic in plain safe Rust (`src/rewrite.rs` there) with
   host tests under `tests/`, and only the ABI glue in `src/lib.rs`.
2. **ABI.** Export `memory`, `alloc` and `transform` (plus
   `export_transform` with `export: true`); import nothing but the declared
   `carrova.log` / `carrova.run_tool` (anything else is refused at load).
   Signatures and the context encoding are in
   `crates/carrova-plugin/src/lib.rs`; return a negative status to refuse a
   file. Tools get no arguments, only stdin; log lines may hold file
   contents, so keep them to short diagnostics.
3. **Permissions.** Declare in `plugin.yaml` only the `context` fields you
   read, and only the tools, `log` and `export` you use. The user sees one
   consent sentence per field (e.g. `target_home` → "It sees the path of
   your home folder on this computer."), per tool, and for `log` and
   `export`, between a fixed "may change these files" line and the sandbox
   guarantee; see `PluginManifest::permissions` in `crates/carrova-plugin/src/manifest.rs`.
   Keep `limits` as low as your largest file allows.
4. **Build.** `just plugins` runs the host tests and builds every
   `plugins/<id>/plugin.wasm` (wasm target from `rust-toolchain.toml`).
   Commit the `.wasm` with its source; `just plugins-check` fails on drift.
5. **Use.** Name it from a schema-3 recipe item: `handler: { plugin: <id> }`.
   Try it unsigned with the `--recipes DIR` override (loads `DIR/../plugins`).
6. **Ship.** `just bundle N` (`cargo xtask bundle build`) packs `plugins/`
   into the signed recipe bundle, then `just bundle-sign` / `just
   bundle-verify`.
7. **Review.** Open a PR; `/plugins/` is core-only in `.github/CODEOWNERS`.

## Review and signing

- Plugins are **core tier only** (`.github/CODEOWNERS`: `/plugins/` →
  core maintainers). There is no community tier for code.
- `run_tools`, `log` and `export` widen the sandbox: a declared tool runs
  unsandboxed with the user's rights, a log line may carry file contents,
  an exporting plugin runs on the sender. Accept one only with a reason.
- Review the source; `just plugins-check` (CI and the release gate) rebuilds
  every `plugin.wasm` and fails if the committed module differs.
- They ship only inside the signed recipe bundle (`cargo xtask bundle build`
  picks up `plugins/` next to `recipes/`), as content-addressed blobs listed
  in the signed index. The `--recipes DIR` developer override loads unsigned
  plugins from `DIR/../plugins`, with the usual unverified warning.
