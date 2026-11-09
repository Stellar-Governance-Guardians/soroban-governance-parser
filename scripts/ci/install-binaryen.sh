#!/usr/bin/env bash
# Pre-seed wasm-pack's binaryen (wasm-opt) cache from a pinned, verified tarball.
#
# wasm-pack downloads binaryen itself with no retry, from the GitHub release CDN
# that intermittently answers 504. When that download fails the whole build job
# fails even though nothing in the repository is wrong. wasm-pack reuses
# $WASM_PACK_CACHE (default ~/.cache/.wasm-pack)/wasm-opt-<key> whenever
# bin/wasm-opt is already there, so seeding that directory removes the CDN from
# the build path.
#
# The key below is wasm-pack's cache key for this exact URL and platform
# (x86_64-linux, binaryen version_117); it is deterministic and was verified by
# a fresh-cache build. wasm-opt itself is unchanged, so the built bytes and the
# release reproducibility check are unaffected.
#
# Inputs (env, all optional except for tests):
#   WASM_PACK_CACHE  cache root (default $HOME/.cache/.wasm-pack)
#   BINARYEN_URL     release URL for the platform tarball
#   BINARYEN_SHA256  sha256 of that tarball (default: the pinned value)
#
# Exit 1 with an ::error:: line on download failure or checksum mismatch.
set -euo pipefail

version="117"
key="${BINARYEN_CACHE_KEY:-1ceaaea8b7b5f7e0}"
cache="${WASM_PACK_CACHE:-$HOME/.cache/.wasm-pack}"
dest="$cache/wasm-opt-${key}"
url="${BINARYEN_URL:-https://github.com/WebAssembly/binaryen/releases/download/version_${version}/binaryen-version_${version}-x86_64-linux.tar.gz}"
want_sha="${BINARYEN_SHA256:-3dc677006555b355ea2da5e82602065a161d5e83eaefd3f759afa00b96e83212}"
asset_id="${BINARYEN_ASSET_ID:-153906727}"

if [ -x "$dest/bin/wasm-opt" ]; then
  echo "wasm-opt ${version} already seeded at $dest/bin/wasm-opt (cache hit)"
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
tarball="$tmp/binaryen-${version}.tar.gz"

downloaded=0
for attempt in 1 2 3; do
  echo "binaryen download attempt ${attempt}/3: $url"
  if curl -sSL --fail --retry 5 --retry-all-errors --retry-delay 5 \
      --connect-timeout 20 --max-time 600 -o "$tarball" "$url"; then
    downloaded=1
    break
  fi
  echo "binaryen download attempt ${attempt}/3 failed" >&2
  [ "$attempt" -lt 3 ] && sleep $((attempt * 10))
done

if [ "$downloaded" -ne 1 ] && [ -n "${GITHUB_TOKEN:-}" ]; then
  echo "release CDN failed 3/3; trying the REST asset endpoint" >&2
  if curl -sSL --fail --retry 3 --retry-all-errors --retry-delay 5 \
      --connect-timeout 20 --max-time 600 \
      -H "Authorization: Bearer ${GITHUB_TOKEN}" \
      -H "Accept: application/octet-stream" \
      -o "$tarball" \
      "https://api.github.com/repos/WebAssembly/binaryen/releases/assets/${asset_id}"; then
    downloaded=1
  fi
fi

if [ "$downloaded" -ne 1 ]; then
  echo "::error::failed to download binaryen ${version} from the release CDN and the REST asset endpoint" >&2
  exit 1
fi

got_sha="$(sha256sum "$tarball" | cut -d' ' -f1)"
if [ "$got_sha" != "$want_sha" ]; then
  echo "::error::binaryen ${version} tarball sha256 mismatch: got ${got_sha}, want ${want_sha}" >&2
  exit 1
fi

tar -xzf "$tarball" -C "$tmp"
wasm_opt="$(find "$tmp" -type f -path '*/bin/wasm-opt' | head -1)"
if [ -z "$wasm_opt" ]; then
  echo "::error::binaryen ${version} tarball did not contain bin/wasm-opt" >&2
  exit 1
fi
mkdir -p "$dest/bin"
install -m 0755 "$wasm_opt" "$dest/bin/wasm-opt"
if [ ! -x "$dest/bin/wasm-opt" ]; then
  echo "::error::wasm-opt is not executable at $dest/bin/wasm-opt after install" >&2
  exit 1
fi
echo "seeded wasm-opt ${version} at $dest/bin/wasm-opt (sha256 verified)"
