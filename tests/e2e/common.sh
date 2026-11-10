# Shared by run.sh (archive e2e) and lan.sh (LAN e2e): build, work dir and
# the selection both e2e flavours migrate. Sourced, not executed.
# shellcheck shell=bash disable=SC2034 # variables are used by the sourcing scripts

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
image=carrova-e2e:ubuntu-24.04

# Sender selection on top of the suggested preset: the SSH keys are
# sensitive, so they are ticked and confirmed explicitly.
sender_selection="--confirm-sensitive ssh/keys --include ssh/keys"

# Receiver-side sensitivity: keys, ssh/gpg, shell rc and git config all need
# explicit confirmation on import (the user decides).
receiver_confirm=""
for k in ssh/keys ssh/config git/config bash/config zsh/config gnupg/public-keyring; do
  receiver_confirm="$receiver_confirm --confirm-sensitive $k"
done

# Builds the CLI (with failpoints) and the image, creates $work with the
# binary in $work/bin and a shared $work/out. Caller owns the EXIT trap.
e2e_setup() {
  cargo build -q -p carrova-cli --features failpoints --manifest-path "$repo/Cargo.toml"
  docker build -q -t "$image" "$here" >/dev/null
  work="$(mktemp -d)"
  mkdir -p "$work/bin" "$work/out"
  cp "$repo/target/debug/carrova" "$work/bin/"
  chmod -R a+rwX "$work"
}
