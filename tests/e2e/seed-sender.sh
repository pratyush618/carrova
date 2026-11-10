#!/usr/bin/env bash
# Runs inside the sender container: builds a realistic dev home.
set -euo pipefail
cd "$HOME"
ssh-keygen -q -t ed25519 -N '' -C e2e@carrova -f ~/.ssh/id_ed25519
printf 'Host example\n  HostName example.com\n  User dev\n' > ~/.ssh/config
chmod 600 ~/.ssh/config
git config --global user.name "Carrova E2E"
git config --global user.email "e2e@carrova.invalid"
git config --global init.defaultBranch main
printf 'export CARROVA_E2E=1\nalias ll="ls -la"\n' >> ~/.bashrc
printf 'export CARROVA_E2E_ZSH=1\nsetopt autocd\n' > ~/.zshrc
mkdir -p ~/.config/Code/User
printf '{ "editor.fontSize": 15, "carrova.e2e": true }\n' > ~/.config/Code/User/settings.json
gpg --batch --quiet --passphrase '' --quick-gen-key 'Carrova E2E <e2e@carrova.invalid>' ed25519 default never
gpg --batch --quiet --list-keys --with-colons | awk -F: '/^fpr/ {print $10; exit}' > /out/gpg-fpr
sha256sum ~/.ssh/id_ed25519 > /out/ssh-key.sha256
