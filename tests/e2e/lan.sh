#!/usr/bin/env bash
# Carrova LAN end-to-end: sender and receiver run concurrently in two
# containers on a private Docker bridge network and migrate over QUIC.
#
#   A   pairing by code only (discovery inside the bridge); a dry run writes
#       nothing; real receive with the archive e2e's selection; verify.
#   B1  network drop mid-transfer (sender disconnected longer than the QUIC
#       idle timeout); the same `carrova receive` run reconnects and finishes.
#   B2  sender container killed mid-transfer, restarted with
#       `carrova send --resume`; the still-running receiver reconnects.
#   B3  receiver process killed mid-transfer; `carrova receive --resume`.
#       B* add a ${E2E_BIG_MIB} MiB raw item and assert from the receiver's
#       interface byte counters that already-received data was not fetched
#       again (a full re-download would exceed E2E_MAX_RX_PCT).
#   C   forced apply failure (--fail-after 3): receiver byte-identical.
#
# Usage: tests/e2e/lan.sh [--only A|B1|B2|B3|C]... [--ubuntu-rmem]
#   (or `just e2e --lan ...`)
#   --ubuntu-rmem  run with Ubuntu's default net.core.{r,w}mem_max (212992).
#                  These sysctls are global, not per network namespace, so a
#                  container cannot set them (`docker run --sysctl` is
#                  refused); this sets them on the HOST via a privileged
#                  container for the run and restores them on exit.
# Env: E2E_BIG_MIB (300), E2E_CUT_PCT (50), E2E_MAX_RX_PCT (125),
#      E2E_DROP_SECS (35), E2E_TIMEOUT (600, seconds per carrova run).
set -euo pipefail
# shellcheck source-path=SCRIPTDIR source=common.sh
. "$(dirname "$0")/common.sh"

big_mib=${E2E_BIG_MIB:-300}
cut_pct=${E2E_CUT_PCT:-50}
max_rx_pct=${E2E_MAX_RX_PCT:-125}
drop_secs=${E2E_DROP_SECS:-35}
t_run=${E2E_TIMEOUT:-600}
ubuntu_rmem=212992

only=()
set_rmem=0
while [ $# -gt 0 ]; do
  case "$1" in
    --only) only+=("${2:?--only needs a scenario}"); shift ;;
    --ubuntu-rmem) set_rmem=1 ;;
    *) echo "usage: $0 [--only A|B1|B2|B3|C]... [--ubuntu-rmem]" >&2; exit 2 ;;
  esac
  shift
done
if [ $((100 + cut_pct)) -le "$max_rx_pct" ]; then
  echo "E2E_MAX_RX_PCT must be below 100 + E2E_CUT_PCT, else a full re-download passes" >&2
  exit 2
fi

run_id=$$
net="carrova-e2e-lan-$run_id"
label="carrova-e2e-lan=$run_id"
# shellcheck disable=SC2088 # expanded inside the container
big_dir='~/.local/share/carrova-e2e-big'
big_item="raw:user_data/carrova-e2e-big" # sender selection key
# Receiver: unrecognised data is sensitive; its item key has no `raw:` prefix.
big_confirm="--confirm-sensitive user_data/carrova-e2e-big"
send_resume="carrova send --resume --json"
receive_resume="carrova receive --resume --yes --json"
scn=""
results=()

# ---- helpers ---------------------------------------------------------------

host_sysctl() { # host_sysctl KEY [VALUE]: read, or write via a privileged container
  local f="/proc/sys/${1//.//}"
  if [ $# = 1 ]; then cat "$f"; return; fi
  docker run --rm --user root --privileged --network host "$image" sh -c "echo $2 > $f"
}

saved_rmem=""
saved_wmem=""
cleanup() {
  docker ps -aq --filter "label=$label" | xargs -r docker rm -f >/dev/null 2>&1 || true
  docker network rm "$net" >/dev/null 2>&1 || true
  if [ -n "$saved_rmem" ]; then
    host_sysctl net.core.rmem_max "$saved_rmem" && host_sysctl net.core.wmem_max "$saved_wmem" \
      || echo "WARNING: restore host sysctls by hand: net.core.rmem_max=$saved_rmem net.core.wmem_max=$saved_wmem" >&2
  fi
  [ -z "${work:-}" ] || rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

fail() {
  echo "E2E FAIL [$scn]: $*" >&2
  local f
  for f in "$work/out/$scn"-*; do
    if [ ! -s "$f" ] || [[ "$f" == *.rc ]]; then continue; fi
    echo "---- $(basename "$f") (last 25 lines)" >&2
    tail -n 25 "$f" >&2
  done
  exit 1
}

pass() { echo "PASS $scn: $*"; results+=("PASS $scn: $*"); }

poll() { # poll SECS CMD...: 0 once CMD succeeds, 1 on timeout
  local end=$((SECONDS + $1)); shift
  until "$@"; do
    [ "$SECONDS" -lt "$end" ] || return 1
    sleep 0.2
  done
}

wait_until() { # wait_until SECS WHAT CMD...: poll or fail
  local secs=$1 what=$2; shift 2
  poll "$secs" "$@" || fail "timed out after ${secs}s waiting for $what"
}

ctr() { echo "carrova-e2e-lan-$run_id-$scn-$1"; }

up() { # up ROLE...: idle containers for the current scenario
  local role
  for role; do
    docker run -d --name "$(ctr "$role")" --label "$label" --network "$net" \
      --hostname "$role" \
      -v "$work/bin:/carrova/bin:ro" -v "$repo/recipes:/carrova/recipes:ro" \
      -v "$here:/e2e:ro" -v "$work/out:/out" "$image" sleep infinity >/dev/null
  done
}

in_ctr() { # in_ctr ROLE SCRIPT: run in the foreground (bounded)
  docker exec "$(ctr "$1")" timeout "$t_run" bash -euo pipefail -c "$2"
}

bg() { # bg ROLE TAG SCRIPT: run detached; output in /out/TAG.{out,err}, exit code in /out/TAG.rc
  # shellcheck disable=SC2016 # expanded by the container's bash
  docker exec -d "$(ctr "$1")" bash -c \
    'timeout "$0" bash -euo pipefail -c "$1" >"/out/$2.out" 2>"/out/$2.err"
     echo $? >"/out/$2.rc.tmp" && mv "/out/$2.rc.tmp" "/out/$2.rc"' "$t_run" "$3" "$2"
}

done_() { [ -f "$work/out/$1.rc" ]; }
rc_of() { cat "$work/out/$1.rc"; }

field() { # field TAG KEY: first string value of KEY in TAG's stdout JSON
  sed -n "s/.*\"$2\": *\"\([^\"]*\)\".*/\1/p" "$work/out/$1.out" 2>/dev/null | head -n 1
}

has_ticket_or_exited() { [ -n "$(field "$1" ticket)" ] || done_ "$1"; }

send_start() { # send_start TAG [EXTRA ARGS]: start `carrova send`, wait for its code
  bg sender "$1" "carrova send $sender_selection ${2:-} --yes --json"
  wait_until 120 "$1 pairing code" has_ticket_or_exited "$1"
  [ -n "$(field "$1" ticket)" ] || fail "$1 exited ($(rc_of "$1")) without printing a pairing code"
}

expect_rc0() { # expect_rc0 TAG SECS
  wait_until "$2" "$1 to exit" done_ "$1"
  [ "$(rc_of "$1")" = 0 ] || fail "$1 exited $(rc_of "$1")"
}

rx_bytes() { # bytes received by the receiver container on all non-loopback interfaces
  # shellcheck disable=SC2016 # expanded by the container's bash
  docker exec "$(ctr receiver)" bash -c \
    'for i in /sys/class/net/*; do [ "${i##*/}" = lo ] || cat "$i/statistics/rx_bytes"; done' \
    | awk '{ s += $1 } END { print s + 0 }'
}

past_cut() { done_ "$1" || [ $(($(rx_bytes) - $2)) -ge "$3" ]; }

begin() { # begin SCENARIO TITLE: fresh sender + receiver
  if [ -n "$scn" ]; then docker rm -f "$(ctr sender)" "$(ctr receiver)" >/dev/null 2>&1 || true; fi
  scn=$1
  echo "== $scn: $2"
  up sender receiver
}

selected() { [ ${#only[@]} = 0 ] || [[ " ${only[*]} " == *" $1 "* ]]; }

seed() { in_ctr sender /e2e/seed-sender.sh || fail "seeding the sender"; }

seed_big() {
  in_ctr sender "/e2e/seed-sender.sh
    mkdir -p $big_dir
    head -c ${big_mib}M /dev/urandom > $big_dir/blob.bin
    sha256sum $big_dir/blob.bin > /out/big.sha256" || fail "seeding the sender"
}

# ---- scenarios -------------------------------------------------------------

scenario_a() {
  begin A "pair by code (discovery), dry run writes nothing, receive, verify"
  seed
  send_start A-send1
  in_ctr receiver "
    /e2e/snapshot.sh > /tmp/before
    carrova receive $(field A-send1 pairing_code) --dry-run $receiver_confirm --json \
      > /out/A-dry.out 2> /out/A-dry.err || { echo 'dry run exited non-zero' >&2; exit 1; }
    /e2e/snapshot.sh > /tmp/after
    diff -u /tmp/before /tmp/after >&2 || { echo 'dry run wrote to the receiver' >&2; exit 1; }
  " || fail "dry run"
  # A dry run pairs and plans but pulls nothing; that pairing is spent.
  if poll 30 done_ A-send1; then
    echo "   sender after the dry run exited with $(rc_of A-send1)"
  else
    echo "   sender still waiting 30 s after the dry run; stopping it"
    docker exec "$(ctr sender)" pkill -x carrova || true
  fi

  send_start A-send2
  in_ctr receiver "carrova receive $(field A-send2 pairing_code) $receiver_confirm --yes --json \
    > /out/A-recv.out 2> /out/A-recv.err" || fail "receive exited non-zero"
  expect_rc0 A-send2 60
  in_ctr receiver /e2e/verify-receiver.sh || fail "receiver verification"
  pass "paired by code via discovery, dry run wrote nothing, migrated setup verified"
}

scenario_b() { # scenario_b B1|B2|B3
  local title
  case $1 in
    B1) title="network drop of ${drop_secs}s mid-transfer, same receive run reconnects" ;;
    B2) title="sender killed mid-transfer, 'carrova send --resume', receiver reconnects" ;;
    B3) title="receiver killed mid-transfer, 'carrova receive --resume'" ;;
  esac
  begin "$1" "$title"
  seed_big
  send_start "$scn-send" "--include $big_item"
  local payload=$((big_mib * 1048576)) rx0 at total
  local cut=$((payload * cut_pct / 100)) send="$scn-send" recv="$scn-recv"
  rx0=$(rx_bytes)
  bg receiver "$recv" "carrova receive $(field "$send" ticket) $receiver_confirm $big_confirm --yes --json"
  wait_until "$t_run" "${cut_pct}% of the payload to arrive" past_cut "$recv" "$rx0" "$cut"
  if done_ "$recv"; then
    fail "receive finished (rc $(rc_of "$recv")) before the interruption; raise E2E_BIG_MIB"
  fi
  at=$(($(rx_bytes) - rx0))
  echo "   interrupting at $((at / 1048576)) of ${big_mib} MiB"

  case $scn in
    B1)
      docker network disconnect "$net" "$(ctr sender)"
      sleep "$drop_secs"
      docker network connect "$net" "$(ctr sender)"
      ;;
    B2)
      docker kill "$(ctr sender)" >/dev/null
      docker start "$(ctr sender)" >/dev/null
      send="$scn-send2"
      bg sender "$send" "$send_resume"
      ;;
    B3)
      docker exec "$(ctr receiver)" pkill -KILL -x carrova || fail "receive was no longer running"
      wait_until 10 "the killed receive to exit" done_ "$recv"
      recv="$scn-recv2"
      bg receiver "$recv" "$receive_resume"
      ;;
  esac

  expect_rc0 "$recv" "$t_run"
  expect_rc0 "$send" 60
  if [ "$scn" != B3 ]; then
    grep -q '"event":"peer_lost"' "$work/out/$recv.err" \
      || fail "receiver never reported the lost peer: reconnect not exercised"
  fi
  total=$(($(rx_bytes) - rx0))
  echo "   receiver got $((total / 1048576)) MiB on the wire for a ${big_mib} MiB payload (limit ${max_rx_pct}%)"
  [ "$total" -ge "$payload" ] || fail "received less than the payload: data did not cross the network"
  [ $((total * 100)) -le $((payload * max_rx_pct)) ] \
    || fail "received $((total * 100 / payload))% of the payload: verified data was fetched again"
  in_ctr receiver "sha256sum -c --quiet /out/big.sha256 && /e2e/verify-receiver.sh" \
    || fail "receiver verification"
  pass "resumed after the interruption, no re-download ($((total * 100 / payload))% of payload on the wire), verified"
}

scenario_c() {
  begin C "forced apply failure rolls back byte-identically"
  seed
  send_start C-send
  in_ctr receiver "
    printf 'old\n' > ~/.zshrc; chmod 640 ~/.zshrc
    git config --global user.name Existing
    /e2e/snapshot.sh > /tmp/before
    if carrova receive $(field C-send ticket) $receiver_confirm --yes --json --fail-after 3 \
        > /out/C-recv.out 2> /out/C-recv.err; then
      echo 'injected failure did not fail' >&2; exit 1
    fi
    /e2e/snapshot.sh > /tmp/after
    diff -u /tmp/before /tmp/after >&2 || { echo 'rollback not byte-identical' >&2; exit 1; }
  " || fail "forced apply failure"
  wait_until 60 "the sender to exit after the receiver's failure" done_ C-send
  pass "mid-apply failure rolled back, home byte-identical incl. modes (sender exited $(rc_of C-send))"
}

# ---- main ------------------------------------------------------------------

e2e_setup
docker network create --label "$label" "$net" >/dev/null

if [ "$set_rmem" = 1 ]; then
  saved_rmem=$(host_sysctl net.core.rmem_max)
  saved_wmem=$(host_sysctl net.core.wmem_max)
  host_sysctl net.core.rmem_max "$ubuntu_rmem"
  host_sysctl net.core.wmem_max "$ubuntu_rmem"
fi
rmem=$(docker run --rm --network "$net" "$image" cat /proc/sys/net/core/rmem_max)
echo "== net.core.rmem_max seen by the containers: $rmem"
if [ "$rmem" != "$ubuntu_rmem" ]; then
  echo "   (not the Ubuntu default $ubuntu_rmem; pass --ubuntu-rmem to test with it)"
fi

if selected A; then scenario_a; fi
for v in B1 B2 B3; do
  if selected "$v"; then scenario_b "$v"; fi
done
if selected C; then scenario_c; fi

if grep -qs 'rmem_max' "$work"/out/*.err; then
  echo "note: carrova warned that the UDP socket buffer was capped (rmem_max)"
fi
[ ${#results[@]} -gt 0 ] || { echo "E2E FAIL: no scenario selected (${only[*]})" >&2; exit 1; }
printf '%s\n' "${results[@]}"
echo "E2E LAN PASSED"
