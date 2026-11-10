# End-to-end tests

| Suite | Entry point | What it proves |
|---|---|---|
| Archive migration (Docker) | `just e2e` → `run.sh` | export → import between clean Ubuntu containers, rollback, forced mid-apply failure |
| LAN migration (Docker) | `just e2e --lan` → `lan.sh` | pairing, interruption + resume, rollback over a Docker network |
| Real desktop app | `just e2e-app` → `apps/desktop/e2e-app/run.sh` | the built app starts and its bridge answers |
| Recipe bundle update | `just e2e-bundle` → `bundle/run.sh` | a new recipe reaches an installed `carrova` without rebuilding it |

Browser flow tests over the fake engine port live in `apps/desktop/e2e/`
(`just e2e-ui`).

## Real desktop app (`apps/desktop/e2e-app/`)

WebdriverIO drives the real Carrova binary through `tauri-driver`, which
proxies WebDriver to `WebKitWebDriver` (the app renders with WebKitGTK), on a
virtual X server (`xvfb-run -a`). Playwright cannot drive WebKitGTK, so this
is the only test of the real app + bridge.

```sh
just e2e-app               # build front-end + app, then run
apps/desktop/e2e-app/run.sh --no-build   # rerun against the last build
```

Prerequisites: `cargo install tauri-driver --locked`, `WebKitWebDriver`
(Ubuntu package `webkit2gtk-driver`), `xvfb`, `just ui-install`.

- **Build:** a plain debug build loads `devUrl` (the Vite dev server). The
  script builds with `--features tauri/custom-protocol`, which embeds
  `apps/desktop/dist` as a release does (prod CSP, `tauri://` origin), into
  its own `target/e2e-app/` so the shared `target/debug` keeps its feature set.
  It is still a debug build, so recipes come from the checkout's `recipes/`.
- **Isolation:** `tauri-driver` (and through it WebKitWebDriver and the app)
  starts with `HOME` and every `XDG_*_HOME` in a fresh temp dir, removed on
  exit; `CARROVA_RECIPES` is dropped. The test asserts the app's state dir was
  created there.
- **Asserts:** one window titled Carrova; the page runs inside Tauri (not the
  fake port); Welcome → Home; Apps lists ≥ 15 recipes from the engine;
  Settings shows the recipe source; no script errors, unhandled rejections,
  `console.error` or CSP violations.
- **Limit:** WebKitWebDriver has no log endpoint, so errors are collected by
  a hook installed after the first paint; errors before it surface as
  screens failing to render.
- **Env:** `CARROVA_E2E_APP` (binary to test), `CARROVA_E2E_DRIVER_PORT`
  (default 4444; WebKitWebDriver uses the next port).

## Recipe bundle update (`bundle/`)

`bundle/run.sh` builds bundles v1 (the repo's recipes) and v2 (v1 + the
throwaway `e2e-bundle-probe` recipe from `bundle/fixtures/recipes/`, never
part of `recipes/`), signs both with a root + signer generated on the fly
(`xtask bundle keygen|certify|sign|verify`), and serves a
GitHub-Releases-like layout from `python3 -m http.server` on a free loopback
port:

```
<base>/recipes-latest/latest.json   {"version": N}   re-pointed atomically to publish
<base>/recipes-v1/…  <base>/recipes-v2/…            immutable bundles
```

Scenario, all in a temp `HOME`/XDG with copies of the binaries (hashed
before and after: no rebuild, no change):

1. publish v1 → `carrova recipes update --url <base>` installs v1 (`status --json`).
2. publish v2 with a tampered `index.json` (probe promoted to the verified
   tier) → refused, v1 kept.
3. publish v2 with a tampered probe blob (the only blob a delta update
   downloads) → refused, v1 kept, probe not listed.
4. publish v2 intact → v2 installed, probe in `carrova recipes list`.
5. `carrova recipes rollback --yes` → v1, probe gone.
6. update again → v2 reported as rolled back and not reinstalled.

```sh
just e2e-bundle
tests/e2e/bundle/run.sh --no-build   # reuse target/debug/{carrova,xtask}
```

**Test seams (pending):** the CLI must trust the test root and accept the
loopback `http://` URL. The script sets `CARROVA_BUNDLE_TEST_ROOTS=<roots
file>` and `CARROVA_BUNDLE_ALLOW_HTTP=1` (names defined once at the top of
`run.sh`). Until the CLI honours them, the run stops at the seam check with a
clear message. They should exist only in debug/test builds (e.g. behind
`cfg(debug_assertions)` or the CLI's `failpoints` feature), never in
release binaries.
