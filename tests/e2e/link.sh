#!/usr/bin/env bash
# shellcheck disable=SC2154 # repo, here, image come from common.sh
# Carrova linked-device e2e: two containers on a private Docker bridge.
# "host" stands in for the app (the CLI never hosts a pairing or answers an
# ask): the test-only cargo example
# `link_host` offers the code, serves the supervisor and accepts the file.
# "guest" is the real CLI end to end: `devices link <ticket>` (pair only),
# `link serve`, `link status`, `devices features`, `file send`; the file
# must arrive byte-identical.
#
# Only the lower device key dials (and only while a feature is on for the
# device); both ends announce themselves, so either key order links.
#
# Clipboard and Handoff are not driven here (no clipboard or desktop in the
# container); in-process tests cover them: crates/carrova-core/tests/ecosystem/
# and apps/cli/tests/{link,devices}/.
#
# Usage: tests/e2e/link.sh   (or `just e2e --link`)
# Env: E2E_TIMEOUT (120, seconds per container command)
set -euo pipefail
# shellcheck source-path=SCRIPTDIR source=common.sh
. "$(dirname "$0")/common.sh"

run_id=$$
net="carrova-e2e-link-$run_id"
label="carrova-e2e-link=$run_id"
t_run=${E2E_TIMEOUT:-120}
out=""

cleanup() {
  docker ps -aq --filter "label=$label" | xargs -r docker rm -f >/dev/null 2>&1 || true
  docker network rm "$net" >/dev/null 2>&1 || true
  [ -z "${work:-}" ] || rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

fail() {
  echo "E2E LINK FAIL: $*" >&2
  local f
  for f in "$out"/*.out "$out"/*.err; do
    [ -s "$f" ] || continue
    echo "---- $(basename "$f") (last 25 lines)" >&2
    tail -n 25 "$f" >&2
  done
  exit 1
}

ctr() { echo "carrova-e2e-link-$run_id-$1"; }
in_ctr() { docker exec "$(ctr "$1")" timeout "$t_run" bash -euo pipefail -c "$2"; }
bg() { docker exec -d "$(ctr "$1")" bash -c "$3 >/out/$2.out 2>/out/$2.err"; }
poll() { # poll SECS CMD...
  local end=$((SECONDS + $1)); shift
  until "$@"; do [ "$SECONDS" -lt "$end" ] || return 1; sleep 0.2; done
}
host_lines() { [ "$(wc -l <"$out/host.out")" -ge "$1" ]; }

up() { # up: fresh host + guest with empty homes
  mkdir -p "$out"; chmod a+rwx "$out"
  local role
  for role in host guest; do
    docker run -d --name "$(ctr "$role")" --label "$label" --network "$net" --hostname "$role" \
      -v "$work/bin:/carrova/bin:ro" -v "$repo/recipes:/carrova/recipes:ro" \
      -v "$here:/e2e:ro" -v "$out:/out" "$image" sleep infinity >/dev/null
  done
}

e2e_setup
out="$work/out"
cargo build -q -p carrova-cli --example link_host --manifest-path "$repo/Cargo.toml"
cp "$repo/target/debug/examples/link_host" "$work/bin/"
chmod -R a+rwX "$work"
docker network create "$net" >/dev/null

echo "== host offers a pair-only code; guest links with the real CLI"
up
bg host host "link_host"
poll 60 host_lines 1 || fail "the host never showed a code"
guest_link="$(in_ctr guest "carrova devices link '$(sed -n 1p "$out/host.out")' --json")" \
  || fail "devices link"
host_fp="$(sed -n 's/.*"linked": *"\([^"]*\)".*/\1/p' <<<"$guest_link")"
[ -n "$host_fp" ] || fail "no host fingerprint in: $guest_link"
poll 30 host_lines 3 || fail "the host did not finish pairing"
echo "   linked with $host_fp"

echo "== guest serves; both sides allow files; guest sends a file"
# A device is dialled only while a feature is on for it, so set that first.
in_ctr guest "carrova devices features '$host_fp' files=ask >/dev/null"
bg guest guest-serve "carrova link serve"
in_ctr guest "
  for _ in \$(seq 100); do
    carrova link status --json | grep -q '\"state\": *\"linked\"' && break
    sleep 0.5
  done
  carrova link status --json | grep -q '\"state\": *\"linked\"'
  head -c 300000 /dev/urandom > /tmp/e2e-payload.bin
  sha256sum /tmp/e2e-payload.bin | cut -d' ' -f1 > /out/payload.sha256
  carrova file send /tmp/e2e-payload.bin --to '$host_fp' --json
" || fail "the link did not come up or the file did not send"

echo "== the file arrived byte-identical on the host"
poll 30 host_lines 4 || fail "the host never saved the file"
saved="$(sed -n 4p "$out/host.out")"
want="$(cat "$out/payload.sha256")"
got="$(in_ctr host "sha256sum '$saved' | cut -d' ' -f1")"
[ "$got" = "$want" ] || fail "host file hash '$got' != sent '$want'"
echo "PASS: $saved on the host matches the guest's file (sha256 ${want:0:12}…)"
echo "E2E LINK PASSED"
