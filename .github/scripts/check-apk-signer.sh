#!/usr/bin/env bash
# Fails unless APK ($1) is signed by exactly the release key the app pins:
# RELEASE_CERT_SHA256 in the Rust source (the one source of truth), compared
# with what apksigner ($2) prints. A phone refuses any other signer, so a
# release signed with another key could never update anyone.
set -euo pipefail

apk="$1"
apksigner="$2"
readonly PIN_SRC=crates/carrova-bundle/src/app_update/mod.rs

pinned="$(sed -n '/RELEASE_CERT_SHA256: \[u8; 32\] = \[/,/\];/p' "$PIN_SRC" |
  grep -o '0x[0-9A-Fa-f]\{2\}' | sed 's/^0x//' | tr -d '\n' | tr 'A-F' 'a-f')"
if [[ ${#pinned} -ne 64 ]]; then
  echo "::error::cannot read RELEASE_CERT_SHA256 from $PIN_SRC"
  exit 1
fi

# One line per signer and scheme ("Signer #1 …" or "V2 Signer: …"): the
# distinct digests must be exactly the pin.
signers="$("$apksigner" verify --print-certs "$apk" |
  sed -n 's/^.*[Ss]igner.* certificate SHA-256 digest: //p' | sort -u | tr '\n' ' ' | sed 's/ $//')"
if [[ "$signers" != "$pinned" ]]; then
  echo "::error::the APK's signer ($signers) is not the pinned release key ($pinned)"
  exit 1
fi
echo "APK signer matches RELEASE_CERT_SHA256"
