#!/usr/bin/env bash
# Real-app smoke: build the front-end, build the debug desktop app with the
# front-end embedded, then drive it with tauri-driver + WebdriverIO under a
# virtual X server. Usage: run.sh [--no-build] [wdio args...]
#
# Needs: tauri-driver (`cargo install tauri-driver --locked`),
# WebKitWebDriver (Ubuntu: webkit2gtk-driver), xvfb-run (xvfb).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"

build=1
if [[ "${1:-}" == "--no-build" ]]; then
  build=0
  shift
fi

for tool in tauri-driver WebKitWebDriver xvfb-run; do
  command -v "$tool" >/dev/null || { echo "e2e-app: $tool not found on PATH" >&2; exit 1; }
done

if ((build)); then
  pnpm --dir "$repo" --filter desktop build
  # A plain debug build loads `devUrl` (the Vite dev server); the
  # `custom-protocol` feature embeds dist/ as a release does. Own target dir,
  # so the shared target/debug keeps its feature set (no rebuild ping-pong).
  CARGO_TARGET_DIR="$repo/target/e2e-app" cargo build \
    --manifest-path "$repo/Cargo.toml" -p carrova-desktop --features tauri/custom-protocol
fi

export CARROVA_E2E_APP="${CARROVA_E2E_APP:-$repo/target/e2e-app/debug/carrova-desktop}"
[[ -x "$CARROVA_E2E_APP" ]] || { echo "e2e-app: no app at $CARROVA_E2E_APP (drop --no-build)" >&2; exit 1; }

# -a: pick a free display number, so parallel runs and a real X session coexist.
exec xvfb-run -a --server-args="-screen 0 1280x900x24" \
  pnpm --dir "$repo" --filter desktop-e2e-app exec wdio run wdio.conf.ts "$@"
