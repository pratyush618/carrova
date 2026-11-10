# Contributing to Carrova

Thanks for helping. Carrova moves people's configs and keys between machines, so
the bar for correctness and safety is high, but the workflow is simple: every
check CI runs is a `just` recipe you can run locally first.

- Writing or changing an app recipe? See [`recipes/CONTRIBUTING.md`](recipes/CONTRIBUTING.md).
- Found a security issue? Do **not** open an issue. See [`SECURITY.md`](SECURITY.md).

## Dev setup (Ubuntu 24.04+)

1. **Rust:** install [rustup](https://rustup.rs). The toolchain is pinned in
   `rust-toolchain.toml` and installed automatically on first `cargo` use.
2. **just:** `cargo install just` (or your package manager).
3. **System libraries** for the Tauri desktop crate (a workspace member, so
   `cargo build --workspace` needs them): `just setup-ubuntu`
   (WebKitGTK 4.1, libsoup 3, JavaScriptCore, librsvg, ayatana-appindicator, libxdo).
4. **Node 24 + pnpm:** pnpm version is pinned by `packageManager` in
   `package.json`; `corepack enable` picks it up. Then `just ui-install`.
5. **Docker** (optional) for the container end-to-end tests.

## Workflows

The `justfile` is the single entry point (`just --list`). CI calls the same
recipes, so green locally means green in CI.

| Command | What it does |
|---|---|
| `just dev [crate]` | fast check + tests for one crate |
| `just build` / `just test` | workspace build / unit + integration tests |
| `just lint` | `fmt --check`, clippy `-D warnings`, rustdoc `-D warnings` |
| `just fixtures [app]` | recipe fixture tests |
| `just ui-lint` / `just ui-test` | eslint + prettier + tsc / Vitest |
| `just e2e-ui` | Playwright flows against the production front-end build |
| `just e2e` / `just e2e --lan` | archive and LAN migrations between Ubuntu containers |
| `just ci` | everything CI runs: **run before opening a PR** |

Add new workflows as `just` recipes, not ad-hoc scripts or CI-only steps.

## Architecture rules (never break)

1. **OS-specific code lives only in `carrova-platform`.** No `cfg(target_os)`,
   `/proc`, XDG or `std::os::unix` elsewhere; add a `Platform` trait method instead.
2. **`src-tauri` holds no business logic.** CLI and desktop behave identically;
   both go through `carrova-core` / `carrova-view`.
3. **Recipes are data only.** No code, no absolute paths, only semantic roots.
4. **Dependencies point down:** front-ends → view → core → services → platform.
   Services never depend on core.

## Code conventions

- Rust edition 2024, stable toolchain, `unsafe_code = "forbid"`.
- Libraries: `thiserror` error enums; no `unwrap`/`expect`/`panic!` outside tests.
  CLI/xtask: `anyhow`.
- Logging via `tracing`. **Never log** file contents, secrets, keys, passphrases
  or pairing codes.
- Small, cohesive modules; types that make invalid states unrepresentable;
  docs on every public item.
- Front-end: TypeScript strict, every user-facing string through i18n, IPC types
  generated from Rust (`ts-rs`), never hand-written.

## Tests

- Bug fix: write a regression test that fails first, then fix.
- Session logic is tested with two engine instances in one process.
- Every recipe ships a fixture and a passing fixture test.

## Commits and pull requests

- One logical change per commit, message per [Commit conventions](#commit-conventions).
  Body only when the *why* isn't obvious from the diff.
- Keep PRs focused; describe what changed and how you tested it.
- `just ci` must be green. Path-filtered GitHub Actions (`engine`, `ui`,
  `recipes`) run on every PR.
- Code owners (`.github/CODEOWNERS`) review their areas. Changes to
  `carrova-crypto`, `carrova-transport`, `carrova-bundle`, `keys/` or `.github/`
  need a core maintainer; handshake or crypto changes need an explicit design
  sign-off in the PR before merge.

## Commit conventions

Commits and PR titles follow [Conventional Commits](https://www.conventionalcommits.org),
checked by commitlint (`commitlint.config.mjs`: allowed scopes, header ≤ 72 chars),
e.g. `feat(recipes): add fish recipe`, `fix(core): reject symlink escape in stage`.

- CI: `.github/workflows/commitlint.yml` lints every commit in a PR;
  `.github/workflows/pr-title.yml` lints the PR title (it becomes the squash commit).
- Locally (pnpm root devDeps, run `pnpm install` first):
  - `just hooks` opts in to the `.githooks/commit-msg` hook;
  - `just commitlint [from] [to]` lints a range;
  - `echo "msg" | just commitlint-msg` lints one message.

## Security rules (summary)

- The receiver normalises every path, rejects escapes from semantic roots and
  never follows symlinks out of them.
- Sensitive items (SSH/GPG keys, tokens) are off by default and need explicit
  confirmation.
- Permissions are restored exactly (e.g. `600` on SSH private keys).
- Nothing is written on the receiver before the dry-run plan is approved; every
  apply is staged, verified, backed up and reversible.
- Crypto uses audited crates only; no hand-rolled primitives.
- Never commit secrets: `*.key` and `.env*` are git-ignored; recipe fixtures use
  dummy data only.

## License

By contributing you agree that your contributions are licensed under the
Apache License 2.0, the license of this project.
