#!/usr/bin/env bash
# Carrova end-to-end: export from a seeded container, import into a clean one,
# verify the dev setup works; then force a mid-apply failure and verify the
# receiver is byte-identical to before (auto rollback), and a manual rollback.
#
# Usage: tests/e2e/run.sh [--archive FILE]   (FILE: use an existing archive
# instead of the seeded sender, e.g. one exported from the dev laptop; set
# CARROVA_PASSPHRASE and E2E_IMPORT_ARGS accordingly; foreign archives are
# checked by verify-generic.sh — keys usable + 600, configs parse, shells
# start, gpg keys present, nothing world/group-writable).
#
#        tests/e2e/run.sh --lan [...]   LAN migration between two containers
#        (see lan.sh).
set -euo pipefail
[ "${1:-}" = --lan ] && exec "$(dirname "$0")/lan.sh" "${@:2}"
# shellcheck source-path=SCRIPTDIR source=common.sh
. "$(dirname "$0")/common.sh"
export CARROVA_PASSPHRASE="${CARROVA_PASSPHRASE:-carrova-e2e-passphrase}"
archive_in=""
[ "${1:-}" = --archive ] && archive_in="$2"

e2e_setup; trap 'rm -rf "$work"' EXIT

run() { # run <name> <script...>
  docker run --rm --name "carrova-e2e-$1-$$" \
    -e CARROVA_PASSPHRASE -e E2E_KEEP_GROUP_WRITE \
    -v "$work/bin:/carrova/bin:ro" -v "$repo/recipes:/carrova/recipes:ro" \
    -v "$here:/e2e:ro" -v "$work/out:/out" "$image" bash -euo pipefail -c "$2"
}

if [ -z "$archive_in" ]; then
  echo "== sender: seed + export"
  run sender "/e2e/seed-sender.sh && carrova export -o /out/home.carrova --kdf-memory 64 $sender_selection --yes"
  import_args="$receiver_confirm"
else
  cp "$archive_in" "$work/out/home.carrova"; chmod a+r "$work/out/home.carrova"
  import_args="${E2E_IMPORT_ARGS:-}"
fi

echo "== receiver: dry run writes nothing, then import + verify"
verify="/e2e/verify-receiver.sh"; [ -n "$archive_in" ] && verify="/e2e/verify-generic.sh"
run receiver "
  /e2e/snapshot.sh > /tmp/before
  carrova import /out/home.carrova --dry-run $import_args
  /e2e/snapshot.sh > /tmp/after-dry
  diff -u /tmp/before /tmp/after-dry || { echo 'E2E FAIL: dry run wrote'; exit 1; }
  carrova import /out/home.carrova $import_args --yes
  $verify
"

# Separate container: verification starts shells/tools that may write files,
# which must not pollute the byte-identical rollback check.
echo "== receiver: import then manual rollback restores the original home"
run rollback "
  /e2e/snapshot.sh > /tmp/before
  carrova import /out/home.carrova $import_args --yes >/dev/null
  carrova rollback --yes
  /e2e/snapshot.sh > /tmp/after-rb
  diff -u /tmp/before /tmp/after-rb || { echo 'E2E FAIL: manual rollback not identical'; exit 1; }
  echo 'E2E OK: manual rollback restored the original home'
"

echo "== receiver: forced mid-apply failure rolls back"
run failure "
  printf 'old\n' > ~/.zshrc; chmod 640 ~/.zshrc
  git config --global user.name Existing
  /e2e/snapshot.sh > /tmp/before
  if carrova import /out/home.carrova $import_args --yes --fail-after 3; then
    echo 'E2E FAIL: injected failure did not fail'; exit 1; fi
  /e2e/snapshot.sh > /tmp/after
  diff -u /tmp/before /tmp/after || { echo 'E2E FAIL: rollback not byte-identical'; exit 1; }
  echo 'E2E OK: mid-apply failure rolled back, home byte-identical incl. modes'
"
echo "E2E PASSED"
