#!/usr/bin/env bash
# Clone (or refresh) the pinned upstream governor sources into ../references/
# and check each one out at the exact SHA recorded in upstream.lock.json. Fail
# closed on any mismatch. This makes build-upstream.sh reproducible from a clean
# clone: references/ is a scratch area and is NOT version-controlled.
#
# Only the repos named in upstream.lock.json are touched. Any unrelated
# checkouts that happen to live in ../references/ (e.g. the stellar-agent-guard-*
# repos) are left alone and are not part of this program's documented flow.
#
# Usage: scripts/upstream/fetch-references.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
REPO_ROOT="$(pwd)"
LOCK="$REPO_ROOT/scripts/upstream/upstream.lock.json"
REF_ROOT="$(cd "$REPO_ROOT/.." && pwd)/references"
mkdir -p "$REF_ROOT"

command -v jq >/dev/null || { echo "FATAL: jq required" >&2; exit 1; }
command -v git >/dev/null || { echo "FATAL: git required" >&2; exit 1; }
[ -f "$LOCK" ] || { echo "FATAL: missing $LOCK" >&2; exit 1; }

while IFS=$'\t' read -r id repo ref_dir sha; do
  dir="$REF_ROOT/$ref_dir"
  if [ ! -d "$dir/.git" ]; then
    echo "== init $id -> $dir"
    git init -q "$dir"
    git -C "$dir" remote add origin "$repo" 2>/dev/null || git -C "$dir" remote set-url origin "$repo"
  else
    echo "== refresh $id"
    git -C "$dir" remote set-url origin "$repo" 2>/dev/null || true
  fi
  # Fetch exactly the pinned commit (GitHub allows fetch-by-SHA), then detach.
  git -C "$dir" fetch --quiet --depth 1 origin "$sha"
  git -C "$dir" checkout --quiet --detach "$sha"
  head_sha=$(git -C "$dir" rev-parse HEAD)
  if [ "$head_sha" != "$sha" ]; then
    echo "FATAL: $id reference checkout is at $head_sha, pinned $sha (fail closed)" >&2
    exit 1
  fi
  echo "   pin ok: $id @ $sha"
done < <(jq -r '.governors[] | [.id, .repo, .refDir, .commitSha] | @tsv' "$LOCK")

echo "REFERENCES OK — pinned upstream sources in $REF_ROOT"
