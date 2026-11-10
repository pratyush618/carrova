#!/usr/bin/env bash
# Recipe bundle update e2e: a new recipe reaches an installed `carrova`
# without rebuilding it (Phase 4 exit gate, CLI half).
#
# Builds bundles v1 and v2 (v2 = repo recipes + a throwaway probe recipe from
# fixtures/), signs both with a trust root generated on the fly, and serves a
# GitHub-Releases-like layout from a loopback HTTP server:
#
#   <base>/recipes-latest/latest.json   {"version": N}   (re-pointed to publish)
#   <base>/recipes-v1/…                 immutable bundle v1
#   <base>/recipes-v2/…                 immutable bundle v2
#
# Then, in a throwaway HOME, drives `carrova recipes update|status|list|rollback`
# through: install v1 → tampered v2 refused (index, then blob) → v2 installed
# → rollback to v1 → v2 not offered again. The carrova binary is hashed before
# and after: it must not change.
#
# Usage: tests/e2e/bundle/run.sh [--no-build]
# Needs: cargo, python3, curl. Never touches the real ~/.config or ~/.local.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
probe=e2e-bundle-probe
timeout_s="${E2E_TIMEOUT:-30}"

# Test seams the CLI must honour so a test root and a loopback http:// URL can
# stand in for the release roots and GitHub (see tests/e2e/README.md).
seam_roots_env=CARROVA_BUNDLE_TEST_ROOTS   # roots file trusted instead of keys/bundle-roots.json
seam_http_env=CARROVA_BUNDLE_ALLOW_HTTP    # =1: accept plain http:// (loopback test server)

build=1
[[ "${1:-}" == "--no-build" ]] && build=0

fail() { echo "E2E FAIL: $*" >&2; exit 1; }
ok() { echo "E2E OK: $*"; }
step() { echo "== $*"; }

if ((build)); then
  cargo build -q --manifest-path "$repo/Cargo.toml" -p carrova-cli -p xtask --features carrova-cli/e2e-seams
fi
carrova_bin="$repo/target/debug/carrova"
xtask_bin="$repo/target/debug/xtask"
[[ -x "$carrova_bin" && -x "$xtask_bin" ]] || fail "missing $carrova_bin or $xtask_bin (drop --no-build)"

work="$(mktemp -d -t carrova-e2e-bundle.XXXXXX)"
server_pid=""
cleanup() {
  if [[ -n "$server_pid" ]]; then kill "$server_pid" 2>/dev/null || true; fi
  rm -rf "$work"
}
trap cleanup EXIT INT TERM

# Copy the binaries out of target/: a concurrent cargo build cannot swap them
# mid-run, and the hash check below sees exactly what ran.
mkdir -p "$work/bin"
cp "$carrova_bin" "$xtask_bin" "$work/bin/"
carrova_sha="$(sha256sum "$work/bin/carrova" | cut -d' ' -f1)"

# Isolated user: every XDG base dir and HOME live under $work.
export HOME="$work/home" XDG_CONFIG_HOME="$work/home/.config" XDG_DATA_HOME="$work/home/.local/share" \
  XDG_STATE_HOME="$work/home/.local/state" XDG_CACHE_HOME="$work/home/.cache"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_STATE_HOME" "$XDG_CACHE_HOME"
unset CARROVA_RECIPES  # a developer override would win over the installed bundle

carrova() { "$work/bin/carrova" "$@"; }
xtask() { "$work/bin/xtask" "$@"; }

# --- Keys: a fresh root certifies a fresh signer (valid 1 day) ---------------
step "keys: generate test root + signer"
keys="$work/keys"
mkdir -p "$keys"
# keygen prints only the public key (hex) on stdout.
root_pub="$(xtask bundle keygen --role root --out "$keys/root.key" | grep -Eo '[0-9a-f]{64}' | head -1)"
signer_pub="$(xtask bundle keygen --role signer --out "$keys/signer.key" | grep -Eo '[0-9a-f]{64}' | head -1)"
[[ -n "$root_pub" && -n "$signer_pub" ]] || fail "keygen printed no public key"
xtask bundle certify --root "$keys/root.key" --signer-pub "$signer_pub" --days 1 --out "$keys/signer.cert.json"
printf '{ "roots": ["%s"] }\n' "$root_pub" > "$keys/roots.json"
# Every carrova command from here on trusts only the test root (the installed
# bundle is re-verified on each load, not just on update).
export "$seam_roots_env=$keys/roots.json" "$seam_http_env=1"

# --- Bundles v1 and v2 -------------------------------------------------------
# v1 = repo recipes; v2 = v1 + the probe recipe (overlaid, never in recipes/).
build_bundle() { # <version> <recipe tree>
  local out="$work/build/v$1"
  xtask bundle build --recipes "$2" --out "$out" --version "$1"
  xtask bundle sign --dir "$out" --signer "$keys/signer.key" --cert "$keys/signer.cert.json"
  xtask bundle verify --dir "$out" --roots "$keys/roots.json"
}
step "bundles: build, sign, verify v1 and v2"
mkdir -p "$work/trees"
cp -r "$repo/recipes" "$work/trees/v1"
cp -r "$work/trees/v1" "$work/trees/v2"
cp -r "$here/fixtures/recipes/." "$work/trees/v2/"
build_bundle 1 "$work/trees/v1"
build_bundle 2 "$work/trees/v2"

# Blob of the probe recipe in v2: the only recipe blob v1 lacks, so the only
# one a delta update downloads (tampering a shared blob would go unnoticed
# by design — it is never fetched).
probe_blob="$(python3 -I -c '
import json, sys
index = json.load(open(sys.argv[1]))
print(next(r["hash"] for r in index["recipes"] if r["id"] == sys.argv[2]))
' "$work/build/v2/index.json" "$probe").yaml"
[[ -f "$work/build/v2/$probe_blob" ]] || fail "probe blob $probe_blob not in bundle v2"

# --- Release host --------------------------------------------------------------
www="$work/www"
mkdir -p "$www/recipes-latest"
publish() { # <version> [source dir]: upload recipes-vN/ (immutable) and re-point latest
  local v="$1" src="${2:-$work/build/v$1}"
  rm -rf "$www/recipes-v$v"
  cp -r "$src" "$www/recipes-v$v"
  # Atomic re-point, as a client may read the pointer at any moment.
  printf '{"version": %s}\n' "$v" > "$www/recipes-latest/latest.json.tmp"
  mv "$www/recipes-latest/latest.json.tmp" "$www/recipes-latest/latest.json"
}

step "server: loopback HTTP on a free port"
# Port 0: the kernel picks a free port; http.server prints it on its first line.
python3 -I -u -m http.server 0 --bind 127.0.0.1 --directory "$www" > "$work/server.log" 2>&1 &
server_pid=$!
port=""
for ((i = 0; i < timeout_s * 10; i++)); do
  port="$(grep -Eo 'port [0-9]+' "$work/server.log" | head -1 | cut -d' ' -f2 || true)"
  [[ -n "$port" ]] && curl -fs -o /dev/null "http://127.0.0.1:$port/" && break
  kill -0 "$server_pid" 2>/dev/null || fail "http server died: $(cat "$work/server.log")"
  sleep 0.1
done
[[ -n "$port" ]] || fail "http server not ready after ${timeout_s}s"
base="http://127.0.0.1:$port/"

installed_version() { # installed bundle version, or "none"
  carrova recipes status --json | python3 -I -c '
import json, sys
s = json.load(sys.stdin)["source"]
print(s["version"] if s["kind"] == "installed" else "none")'
}
expect_version() { # <want> <what>
  local got
  got="$(installed_version)"
  [[ "$got" == "$1" ]] || fail "$2: installed bundle is $got, expected $1"
}
lists_probe() { carrova recipes list | grep -qw -- "$probe"; }

# --- Scenario -------------------------------------------------------------------
step "seam check: CLI accepts the test root and an http:// URL"
publish 1
if ! carrova recipes update --url "$base" > "$work/out" 2>&1; then
  cat "$work/out" >&2
  fail "carrova recipes update cannot use the test root over http:// — test seams" \
    "$seam_roots_env / $seam_http_env missing (pending, see tests/e2e/README.md)"
fi

step "v1 published → update installs v1"
expect_version 1 "after first update"
lists_probe && fail "probe recipe present in v1"
ok "v1 installed"

step "v2 published with a tampered index → refused, v1 kept"
publish 2
cp "$work/build/v2/index.json" "$work/index.orig"
# Promote the community probe to the verified tier: a meaningful attack, and
# the signature covers the raw index bytes.
sed -i 's/"tier": *"community"/"tier": "verified"/' "$www/recipes-v2/index.json"
cmp -s "$work/index.orig" "$www/recipes-v2/index.json" && fail "index tamper changed nothing"
carrova recipes update --url "$base" && fail "update accepted a tampered index"
expect_version 1 "after tampered index"
ok "tampered index refused"

step "v2 published with a tampered probe blob → refused, v1 kept"
publish 2
printf '# tampered\n' >> "$www/recipes-v2/$probe_blob"
carrova recipes update --url "$base" && fail "update accepted a tampered blob"
expect_version 1 "after tampered blob"
lists_probe && fail "probe recipe visible after a refused update"
ok "tampered blob refused"

step "v2 published intact → update installs v2, probe recipe listed"
publish 2
carrova recipes update --url "$base" | tee "$work/out"
expect_version 2 "after v2 update"
lists_probe || fail "probe recipe missing from 'carrova recipes list' after v2"
ok "v2 installed, $probe listed"

step "rollback → v1, probe gone"
carrova recipes rollback --yes
expect_version 1 "after rollback"
lists_probe && fail "probe recipe still listed after rollback"
ok "rolled back to v1"

step "update after rollback → v2 not offered again"
carrova recipes update --url "$base" | tee "$work/out"
grep -q "rolled back earlier" "$work/out" || fail "update did not report v2 as refused"
expect_version 1 "after post-rollback update"
ok "v2 stays refused"

step "binary unchanged"
[[ "$(sha256sum "$work/bin/carrova" | cut -d' ' -f1)" == "$carrova_sha" ]] || fail "carrova binary changed"
ok "same carrova binary throughout"

echo "E2E PASSED (bundle update)"
