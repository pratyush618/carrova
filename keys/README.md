# Recipe bundle trust roots

`bundle-roots.json` lists the Ed25519 **root** public keys that Carrova trusts
for recipe bundles. It is embedded into every build (`carrova_bundle::release_roots`).
Format: `{ "roots": ["<64 hex digits>", ...] }`; rotation = add the new key,
ship a release, later remove the old one.

> **WARNING — DEVELOPMENT ROOT.** The key currently listed was generated on a
> developer laptop for Phase 4 development (secret at
> `~/.config/carrova-dev/bundle-root.key`, outside the repo). It **must be
> replaced by the maintainer's offline root key before any public release.**

## Chain

1. Offline root key certifies a short-lived (≤ 90 days) bundle-signer key:
   `cargo xtask bundle certify --root ROOT.key --signer-pub HEX --days 90 --out signer.cert.json`
2. CI holds only the signer key + certificate and signs each bundle:
   `cargo xtask bundle sign --dir DIR --signer SIGNER.key --cert signer.cert.json`
3. Clients verify the chain against this file:
   `cargo xtask bundle verify --dir DIR --roots keys/bundle-roots.json`

Secret key files (`*.key`) are never committed. Generate them with
`cargo xtask bundle keygen --role root|signer --out FILE` (created mode 0600;
only the public key is printed).
