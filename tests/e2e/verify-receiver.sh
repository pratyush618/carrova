#!/usr/bin/env bash
# Runs inside the receiver container after import: the dev setup must work.
set -euo pipefail
fail() { echo "E2E FAIL: $*" >&2; exit 1; }
cd "$HOME"
# SSH: key bytes and exact modes.
(cd / && sha256sum -c /out/ssh-key.sha256 >/dev/null) || fail "ssh key differs"
[ "$(stat -c %a ~/.ssh/id_ed25519)" = 600 ] || fail "ssh key mode $(stat -c %a ~/.ssh/id_ed25519)"
[ "$(stat -c %a ~/.ssh/config)" = 600 ] || fail "ssh config mode"
ssh-keygen -y -f ~/.ssh/id_ed25519 >/dev/null || fail "ssh key unusable"
ssh -G example | grep -q '^hostname example.com$' || fail "ssh config not applied"
# git
[ "$(git config --global user.email)" = e2e@carrova.invalid ] || fail "git config"
# shells
bash -ic 'test "$CARROVA_E2E" = 1' 2>/dev/null || fail "bashrc"
skip_global_compinit=1 zsh -ic 'test "$CARROVA_E2E_ZSH" = 1' 2>/dev/null || fail "zshrc"
# editor
grep -q '"carrova.e2e": true' ~/.config/Code/User/settings.json || fail "vscode settings"
# gpg (public keyring via gpg-keyring handler)
gpg --batch --list-keys "$(cat /out/gpg-fpr)" >/dev/null 2>&1 || fail "gpg public key missing"
echo "E2E OK: ssh, git, bash, zsh, vscode, gpg verified"
