#!/usr/bin/env bash
# Build the pinned upstream governors and (re)publish the committed wasm
# fixtures. Process rule 6: reference repos must sit exactly at the pinned SHAs
# (fail closed), artifacts must hash exactly as recorded (fail closed unless
# --update-lock is passed in a PR that explains the drift).
#
# Usage:
#   scripts/upstream/build-upstream.sh             # verify pins + rebuild + verify hashes
#   scripts/upstream/build-upstream.sh --update-lock  # rewrite lock hashes after a documented rebuild
#
# This script is a LOCAL/OPERATOR tool: it needs the sibling ../references
# checkouts of the upstream repos. CI never runs it; CI instead verifies that
# the committed wasm fixtures hash-match upstream.lock.json and deployments.json.
set -euo pipefail
cd "$(dirname "$0")/../.."

UPDATE_LOCK=0
[ "${1:-}" = "--update-lock" ] && UPDATE_LOCK=1

LOCK=scripts/upstream/upstream.lock.json
OUT_DIR=tests/fixtures/seed-v2/wasms
REF_ROOT="$(cd .. && pwd)/references"
mkdir -p "$OUT_DIR"

command -v jq >/dev/null || { echo "FATAL: jq required" >&2; exit 1; }

# 1. verify pins
while IFS=$'\t' read -r id ref_dir sha; do
  dir="$REF_ROOT/$ref_dir"
  if [ ! -d "$dir/.git" ]; then
    echo "FATAL: missing reference checkout $dir (clone $id into ../references)" >&2
    exit 1
  fi
  head_sha=$(git -C "$dir" rev-parse HEAD)
  if [ "$head_sha" != "$sha" ]; then
    echo "FATAL: $id reference checkout is at $head_sha, pinned $sha (fail closed)" >&2
    exit 1
  fi
  # Untracked build outputs (target/) are expected; only tracked-file edits fail.
  dirty=$(git -C "$dir" status --porcelain --untracked-files=no | head -5 || true)
  if [ -n "$dirty" ]; then
    echo "FATAL: $id reference checkout has local modifications (fail closed):" >&2
    echo "$dirty" >&2
    exit 1
  fi
  echo "pin ok: $id @ $sha"
done < <(jq -r '.governors[] | [.id, .refDir, .commitSha] | @tsv' "$LOCK")

# 2. build (recipes are executed from each reference checkout root)
build_script3() {
  local dir="$REF_ROOT/soroban-governor"
  (cd "$dir" && cargo +1.81.0 build --release --target wasm32-unknown-unknown \
      -p soroban-votes --no-default-features --features sep-0041)
  # contractimport! embeds the votes wasm: make sure it sits where the governor
  # crate resolves ../../target (mirrors upstream Makefile build ordering).
  mkdir -p "$dir/contracts/governor/target/wasm32-unknown-unknown/release"
  cp "$dir/target/wasm32-unknown-unknown/release/soroban_votes.wasm" \
     "$dir/contracts/governor/target/wasm32-unknown-unknown/release/"
  cp "$dir/target/wasm32-unknown-unknown/release/soroban_votes.wasm" \
     "$dir/contracts/target/wasm32-unknown-unknown/release/" 2>/dev/null || true
  (cd "$dir" && cargo +1.81.0 build --release --target wasm32-unknown-unknown -p soroban-governor)
  (cd "$dir" && cargo +1.81.0 build --release --target wasm32-unknown-unknown -p mock-subcall)
}

build_oz() {
  local dir="$REF_ROOT/stellar-contracts"
  (cd "$dir/examples/fungible-governor/token" && stellar contract build)
  (cd "$dir/examples/fungible-governor/governor" && stellar contract build)
  (cd "$dir/examples/upgradeable/v1" && stellar contract build)
}

echo "== building script3-soroban-governor =="
build_script3
echo "== building openzeppelin-stellar-contracts =="
build_oz

# 3. collect artifacts
declare -A SRC=(
  [script3-soroban-governor.wasm]="$REF_ROOT/soroban-governor/target/wasm32-unknown-unknown/release/soroban_governor.wasm"
  [script3-soroban-votes.wasm]="$REF_ROOT/soroban-governor/target/wasm32-unknown-unknown/release/soroban_votes.wasm"
  [script3-mock-subcall.wasm]="$REF_ROOT/soroban-governor/target/wasm32-unknown-unknown/release/mock_subcall.wasm"
  [oz-fungible-governor-contract.wasm]="$REF_ROOT/stellar-contracts/target/wasm32v1-none/release/fungible_governor_contract.wasm"
  [oz-fungible-governor-token.wasm]="$REF_ROOT/stellar-contracts/target/wasm32v1-none/release/fungible_governor_token.wasm"
  [oz-upgradeable-v1-example.wasm]="$REF_ROOT/stellar-contracts/target/wasm32v1-none/release/upgradeable_v1_example.wasm"
)

status=0
for f in "${!SRC[@]}"; do
  src="${SRC[$f]}"
  [ -f "$src" ] || { echo "FATAL: missing build output $src" >&2; status=1; continue; }
  cp "$src" "$OUT_DIR/$f"
  actual=$(sha256sum "$OUT_DIR/$f" | cut -d' ' -f1)
  pinned=$(jq -r --arg f "$f" '.governors[].artifacts[] | select(.file==$f) | .sha256' "$LOCK")
  if [ "$UPDATE_LOCK" = "1" ]; then
    tmp=$(mktemp)
    jq --arg f "$f" --arg h "$actual" '(.governors[].artifacts[] | select(.file==$f) | .sha256) = $h' "$LOCK" > "$tmp" && mv "$tmp" "$LOCK"
    echo "updated lock: $f -> $actual"
  elif [ "$actual" != "$pinned" ]; then
    echo "FATAL: $f hash drift: built $actual, pinned $pinned" >&2
    echo "  (rebuilds must be byte-reproducible; if upstream/toolchain legitimately moved, rerun with --update-lock in a PR that explains why)" >&2
    status=1
  else
    echo "hash ok: $f $actual"
  fi
done

[ "$status" = "0" ] || exit "$status"
echo "UPSTREAM BUILD OK — fixtures in $OUT_DIR"
