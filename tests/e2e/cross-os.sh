#!/usr/bin/env bash
# Carrova cross-OS e2e: a Linux sender exports the seeded home; receivers
# pretending to be Windows, then macOS (CARROVA_E2E_OS, CLI feature
# `e2e-seams`: fake root layout + permission model under $HOME, real files)
# import it. Proves: VS Code settings land where that OS keeps them,
# byte-identical; a dry run writes nothing; rollback restores the home
# byte-identical; a same_os item is blocked until --allow-cross-os.
#
# A Linux container stands in for Windows/macOS: layouts and permission
# model are faked, the filesystem is not NTFS/APFS (real-hardware gate).
#
# Usage: tests/e2e/cross-os.sh   (or `just e2e --cross-os`)
set -euo pipefail
# shellcheck source-path=SCRIPTDIR source=common.sh
. "$(dirname "$0")/common.sh"
export CARROVA_PASSPHRASE="${CARROVA_PASSPHRASE:-carrova-e2e-passphrase}"

e2e_setup; trap 'rm -rf "$work"' EXIT

echo "== sender (linux): seed + export"
unset CARROVA_E2E_OS
run sender "/e2e/seed-sender.sh
  cp ~/.config/Code/User/settings.json /out/vscode-settings.json
  carrova export -o /out/home.carrova --kdf-memory 64 $sender_selection --yes"

# Where VS Code keeps its user settings under each OS's user_config root.
declare -A settings_dir=(
  [windows]="AppData/Roaming/Code/User"
  [macos]="Library/Application Support/Code/User"
)

for os in windows macos; do
  export CARROVA_E2E_OS="$os"
  echo "== receiver ($os): dry run, import, rollback, same_os override"
  run "receiver-$os" "
    archive=/out/home.carrova
    settings=~/'${settings_dir[$os]}/settings.json'
    vscode='--item vscode/settings'
    bash_rc='--item bash/config --confirm-sensitive bash/config'

    /e2e/snapshot.sh > /tmp/before
    carrova import \$archive --dry-run \$vscode
    /e2e/snapshot.sh > /tmp/after-dry
    diff -u /tmp/before /tmp/after-dry || { echo 'E2E FAIL: dry run wrote'; exit 1; }

    carrova import \$archive \$vscode --yes
    cmp /out/vscode-settings.json \"\$settings\" \
      || { echo 'E2E FAIL: settings not at the $os location or not identical'; exit 1; }
    [ ! -e ~/.config/Code ] || { echo 'E2E FAIL: Linux layout used on $os'; exit 1; }
    echo 'E2E OK ($os): VS Code settings restored byte-identical at the $os location'

    carrova rollback --yes
    /e2e/snapshot.sh > /tmp/after-rb
    diff -u /tmp/before /tmp/after-rb || { echo 'E2E FAIL: rollback not identical'; exit 1; }
    echo 'E2E OK ($os): rollback restored the original home'

    if carrova import \$archive \$bash_rc --yes; then
      echo 'E2E FAIL: same_os bash/config applied without --allow-cross-os'; exit 1; fi
    /e2e/snapshot.sh > /tmp/after-blocked
    diff -u /tmp/before /tmp/after-blocked || { echo 'E2E FAIL: blocked import wrote'; exit 1; }
    carrova import \$archive \$bash_rc --allow-cross-os bash/config --yes
    grep -q CARROVA_E2E=1 ~/.bashrc || { echo 'E2E FAIL: override did not restore bash/config'; exit 1; }
    echo 'E2E OK ($os): same_os item blocked until --allow-cross-os'
  "
done
echo "E2E CROSS-OS PASSED"
