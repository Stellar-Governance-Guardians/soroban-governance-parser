#!/usr/bin/env bash
# Build the parser WASM package for both wasm-pack targets into one .tgz and
# (re)write the release lock file with its sha256.
#
# Usage: scripts/wasm/package-release.sh [version]   (default 0.1.0-alpha.1)
#
# The archive is made deterministic where tar/gzip allow it (sorted names,
# zeroed mtimes/ownership, `gzip -n` so no timestamp is embedded), so the same
# inputs produce the same sha256 on any machine with the same toolchain. The
# lock file records the exact bytes shipped; the README claim checks that the
# README's stated sha256 matches this file.
set -euo pipefail

VERSION="${1:-0.1.0-alpha.1}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

ASSET="sgg-parser-wasm-${VERSION}.tgz"
LOCK="releases/parser-wasm.lock.json"

echo "==> building nodejs + web wasm-pack targets"
wasm-pack build crates/wasm --target nodejs --out-dir pkg/nodejs --out-name sgp_parser_wasm >/dev/null
wasm-pack build crates/wasm --target web --out-dir pkg/web --out-name sgp_parser_wasm >/dev/null

echo "==> assembling ${ASSET}"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/sgg-parser-wasm-${VERSION}/nodejs" "$stage/sgg-parser-wasm-${VERSION}/web"
cp crates/wasm/pkg/nodejs/* "$stage/sgg-parser-wasm-${VERSION}/nodejs/"
cp crates/wasm/pkg/web/* "$stage/sgg-parser-wasm-${VERSION}/web/"
# wasm-pack writes a `.gitignore` containing `*` into each pkg dir; it is an
# artifact of the output dir, not part of the package.
rm -f "$stage/sgg-parser-wasm-${VERSION}/nodejs/.gitignore" "$stage/sgg-parser-wasm-${VERSION}/web/.gitignore"

tar --sort=name --mtime='@0' --owner=0 --group=0 --numeric-owner \
  -C "$stage" -cf - "sgg-parser-wasm-${VERSION}" | gzip -n > "$ASSET"

SHA="$(sha256sum "$ASSET" | cut -d' ' -f1)"
BYTES="$(stat -c '%s' "$ASSET")"

mkdir -p releases
cat > "$LOCK" <<EOF
{
  "\$comment": "Release lock for the parser WASM package. The README states this sha256; scripts/check-wasm-release.sh checks the two agree, offline. The asset is the GitHub Release asset, not committed to git.",
  "version": "${VERSION}",
  "tag": "v${VERSION}",
  "assetName": "${ASSET}",
  "assetUrl": "https://github.com/Stellar-Governance-Guardians/soroban-governance-parser/releases/download/v${VERSION}/${ASSET}",
  "sha256": "${SHA}",
  "bytes": ${BYTES},
  "targets": ["nodejs", "web"],
  "schemaVersion": "v1",
  "toolchain": "$(rustc --version) / wasm-pack $(wasm-pack --version | awk '{print $2}')"
}
EOF

echo "asset  : ${ASSET}"
echo "sha256 : ${SHA}"
echo "lock   : ${LOCK}"
