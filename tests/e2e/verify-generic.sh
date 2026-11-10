#!/usr/bin/env bash
# Runs inside the receiver container after importing a *real* archive (e.g. the
# dev laptop's): checks that whatever arrived actually works, without knowing
# its contents. Prints one line per check; exits non-zero on any failure.
set -uo pipefail
cd "$HOME"
fails=0
ok()   { echo "  ok    $*"; }
bad()  { echo "  FAIL  $*"; fails=$((fails + 1)); }
note() { echo "  note  $*"; }

echo "== verify (generic, real archive)"
# Nothing world-writable, ever; group-write only with --keep-group-write.
ww=$(find "$HOME" -path "$HOME/.local/state" -prune -o -perm -o+w ! -type l -print | head -5)
[ -z "$ww" ] && ok "no world-writable paths" || bad "world-writable: $ww"
if [ "${E2E_KEEP_GROUP_WRITE:-0}" != 1 ]; then
  gw=$(find "$HOME" -path "$HOME/.local/state" -prune -o -perm -g+w ! -type l -print | head -5)
  [ -z "$gw" ] && ok "no group-writable paths" || bad "group-writable without --keep-group-write: $gw"
fi

# SSH: dir 700, every private key 600 and matching its .pub.
if [ -d ~/.ssh ]; then
  [ "$(stat -c %a ~/.ssh)" = 700 ] && ok "~/.ssh mode 700" || bad "~/.ssh mode $(stat -c %a ~/.ssh)"
  for pub in ~/.ssh/*.pub; do
    [ -e "$pub" ] || continue
    key="${pub%.pub}"; [ -f "$key" ] || continue
    name=$(basename "$key"); mode=$(stat -c %a "$key")
    [ "$mode" = 600 ] || { bad "ssh key $name mode $mode"; continue; }
    if derived=$(ssh-keygen -y -P '' -f "$key" 2>/dev/null); then
      [ "$(echo "$derived" | cut -d' ' -f1,2)" = "$(cut -d' ' -f1,2 "$pub")" ] \
        && ok "ssh key $name usable, matches .pub, mode 600" || bad "ssh key $name does not match its .pub"
    else
      ssh-keygen -l -f "$pub" >/dev/null 2>&1 && ok "ssh key $name passphrase-protected, .pub valid, mode 600" \
        || bad "ssh key $name unreadable"
    fi
  done
  if [ -f ~/.ssh/config ]; then
    ssh -G localhost >/dev/null 2>&1 && ok "ssh config parses" || bad "ssh config does not parse"
  fi
fi

# git
if [ -f ~/.gitconfig ] || [ -f ~/.config/git/config ]; then
  git config --global --list >/dev/null 2>&1 && ok "git global config parses" || bad "git config broken"
  e=$(git config --global user.email 2>/dev/null) && [ -n "$e" ] && ok "git user.email set" || note "git user.email not set"
fi

# shells: start interactively and exit cleanly (errors from missing frameworks
# such as an uninstalled oh-my-zsh are reported, not fatal: apps are reinstalled).
for sh in bash zsh; do
  rc=".${sh}rc"; [ -f "$HOME/$rc" ] || continue
  command -v "$sh" >/dev/null || { note "$sh not installed"; continue; }
  err=$(skip_global_compinit=1 "$sh" -ic 'exit 0' 2>&1 >/dev/null </dev/null); status=$?
  [ $status = 0 ] && ok "$sh starts with imported $rc" || bad "$sh exits $status with imported $rc"
  [ -n "$err" ] && note "$sh stderr: $(echo "$err" | head -2 | tr '\n' ' ')"
done

# editor
s=~/.config/Code/User/settings.json
[ -f "$s" ] && { [ -s "$s" ] && ok "vscode settings.json present" || bad "vscode settings.json empty"; }

# gpg (public keyring via the gpg-keyring handler)
if [ -d ~/.gnupg ]; then
  n=$(gpg --batch --list-keys --with-colons 2>/dev/null | grep -c '^pub')
  [ "$n" -gt 0 ] && ok "gpg: $n public key(s) imported" || bad "gpg: no public keys"
fi

[ $fails = 0 ] && echo "E2E OK: real archive verified" || { echo "E2E FAIL: $fails check(s) failed"; exit 1; }
