#!/usr/bin/env bash
# Install wasm-pack from a pinned release asset into ~/.wasm-pack.
#
# Why this exists: jetli/wasm-pack-action downloads the same tarball with no
# retry, so a transient GitHub release-CDN 504 turned the `main` CI red even
# though every check that touches code had passed. This script keeps the same
# pinned version (the built bytes must not change: the release reproducibility
# check depends on them) but retries the download, verifies the sha256 of the
# tarball, and is no-op when a cached copy is already present.
#
# Inputs (env):
#   WASM_PACK_VERSION  required, e.g. 0.13.1
#   WASM_PACK_SHA256   required, sha256 of the .tar.gz for x86_64-linux-musl
#   WASM_PACK_HOME     optional, defaults to $HOME/.wasm-pack
#   WASM_PACK_URL      optional, defaults to the GitHub release URL (tests use file://)
#
# Exit 1 with an explicit ::error:: line on download failure, checksum mismatch,
# or a missing binary. Never falls back to an unverified download.
set -euo pipefail

version="${WASM_PACK_VERSION:?WASM_PACK_VERSION is required}"
want_sha="${WASM_PACK_SHA256:?WASM_PACK_SHA256 is required}"
dest="${WASM_PACK_HOME:-$HOME/.wasm-pack}"
triple="x86_64-unknown-linux-musl"
artifact="wasm-pack-v${version}-${triple}"
url="${WASM_PACK_URL:-https://github.com/rustwasm/wasm-pack/releases/download/v${version}/${artifact}.tar.gz}"

if [ -x "$dest/bin/wasm-pack" ] && "$dest/bin/wasm-pack" --version 2>/dev/null | grep -q "$version"; then
  echo "wasm-pack $version already installed at $dest/bin/wasm-pack (cache hit)"
else
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  downloaded=0
  for attempt in 1 2 3; do
    echo "download attempt ${attempt}/3: $url"
    if curl -sSL --fail --retry 5 --retry-all-errors --retry-delay 5 \
        --connect-timeout 20 --max-time 300 \
        -o "$tmp/${artifact}.tar.gz" "$url"; then
      downloaded=1
      break
    fi
    echo "download attempt ${attempt}/3 failed" >&2
    if [ "$attempt" -lt 3 ]; then sleep $((attempt * 10)); fi
  done
  if [ "$downloaded" -ne 1 ]; then
    # The release CDN (github.com/.../releases/download/...) intermittently
    # returns 504 while the REST asset endpoint keeps serving the same bytes.
    # Fall back to it when a token is available, then verify the same checksum.
    if [ -n "${GITHUB_TOKEN:-}" ]; then
      echo "release CDN failed 3/3; trying the REST asset endpoint" >&2
      api="https://api.github.com/repos/wasm-bindgen/wasm-pack/releases/tags/v${version}"
      asset_id="$(curl -sSL --fail --retry 3 --retry-all-errors --max-time 60 \
        -H "Authorization: Bearer ${GITHUB_TOKEN}" \
        -H "Accept: application/vnd.github+json" "$api" 2>/dev/null \
        | jq -r --arg n "$(basename "$url")" '.assets[]? | select(.name == $n) | .id' 2>/dev/null | head -1)"
      if [ -n "${asset_id:-}" ] && [ "$asset_id" != "null" ]; then
        for attempt in 1 2 3; do
          if curl -sSL --fail --retry 3 --retry-all-errors --retry-delay 5 \
              --connect-timeout 20 --max-time 300 \
              -H "Authorization: Bearer ${GITHUB_TOKEN}" \
              -H "Accept: application/octet-stream" \
              -o "$tmp/${artifact}.tar.gz" \
              "https://api.github.com/repos/wasm-bindgen/wasm-pack/releases/assets/${asset_id}"; then
            downloaded=1
            break
          fi
          echo "REST asset attempt ${attempt}/3 failed" >&2
          [ "$attempt" -lt 3 ] && sleep $((attempt * 10))
        done
      else
        echo "could not resolve the REST asset id" >&2
      fi
    fi
  fi
  if [ "$downloaded" -ne 1 ]; then
    echo "::error::failed to download wasm-pack ${version} from the release CDN and the REST asset endpoint" >&2
    exit 1
  fi

  got_sha="$(sha256sum "$tmp/${artifact}.tar.gz" | cut -d' ' -f1)"
  if [ "$got_sha" != "$want_sha" ]; then
    echo "::error::wasm-pack ${version} tarball sha256 mismatch: got ${got_sha}, want ${want_sha}" >&2
    exit 1
  fi

  tar -xzf "$tmp/${artifact}.tar.gz" -C "$tmp"
  if [ ! -f "$tmp/${artifact}/wasm-pack" ]; then
    echo "::error::wasm-pack ${version} tarball did not contain ${artifact}/wasm-pack" >&2
    exit 1
  fi
  mkdir -p "$dest/bin"
  install -m 0755 "$tmp/${artifact}/wasm-pack" "$dest/bin/wasm-pack"
  echo "installed wasm-pack ${version} to $dest/bin/wasm-pack (sha256 verified)"
fi

if [ ! -x "$dest/bin/wasm-pack" ]; then
  echo "::error::wasm-pack is not executable at $dest/bin/wasm-pack after install" >&2
  exit 1
fi

if [ -n "${GITHUB_PATH:-}" ]; then
  echo "$dest/bin" >> "$GITHUB_PATH"
else
  echo "add to PATH: $dest/bin"
fi
"$dest/bin/wasm-pack" --version
