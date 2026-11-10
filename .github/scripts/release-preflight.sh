#!/usr/bin/env bash
# Refuse to release while development placeholders are still in the tree:
# the OWNER/REPO repository slug, the development recipe-bundle trust root
# and the development updater key. Run before every release build
# (release.yml `verify` job, `just release-preflight` locally).
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

# Public halves of the development keys generated on the dev laptop in
# Phase 4 (secrets in ~/.config/carrova-dev/). keys/bundle-roots.json has no
# machine-readable "dev" marker, so the values are pinned here.
readonly DEV_BUNDLE_ROOT=b05846ea5bcbfa50b569332fed7f06f2978f41d7015d15410e7e3297950698b0
readonly DEV_UPDATER_PUBKEY=dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDkxREFEM0E3QkI5NENEOTAKUldTUXpaUzdwOVBha1N4TVBJb2FrdGRhNHhaRm9ZVjRidHUxUC9xdkI4bXJqNGNPcmhFRTFwMnkK

readonly TAURI_CONF=apps/desktop/src-tauri/tauri.conf.json
readonly SLUG_FILES=(
  "$TAURI_CONF"
  crates/carrova-report/src/issue.rs
  crates/carrova-bundle/src
  apps/cli/Cargo.toml
)

failures=0
fail() {
  echo "::error::$*"
  failures=$((failures + 1))
}

# Placeholder slug in code/config; comment lines (//, #) may still mention it.
for path in "${SLUG_FILES[@]}"; do
  [[ -e "$path" ]] || { fail "$path is missing (update release-preflight.sh)"; continue; }
  while IFS= read -r hit; do
    fail "placeholder OWNER/REPO in $hit"
  done < <(grep -rnH 'OWNER/REPO' "$path" | grep -vE '^[^:]+:[0-9]+:[[:space:]]*(//|#)' || true)
done

if grep -qF "$DEV_BUNDLE_ROOT" keys/bundle-roots.json; then
  fail "keys/bundle-roots.json still trusts the development root key (see keys/README.md)"
fi

if grep -qF "$DEV_UPDATER_PUBKEY" "$TAURI_CONF"; then
  fail "$TAURI_CONF still uses the development updater public key (see packaging/README.md)"
fi

if ((failures > 0)); then
  echo "release preflight: $failures problem(s); fix them before releasing" >&2
  exit 1
fi
echo "release preflight: ok"
