#!/usr/bin/env bash
# Generate fuzz seed corpora from committed fixtures (real shapes, no network).
#
#   scval_from_xdr : the raw XDR bytes of captured ScVal return values
#   parse_spec     : the committed upstream .wasm artifacts
#
# Idempotent: it clears and repopulates the two corpus dirs.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

RPC="$ROOT/tests/fixtures/seed-v2/rpc"
WASMS="$ROOT/tests/fixtures/seed-v2/wasms"
SCVAL_CORPUS="$ROOT/fuzz/corpus/scval_from_xdr"
SPEC_CORPUS="$ROOT/fuzz/corpus/parse_spec"

rm -rf "$SCVAL_CORPUS" "$SPEC_CORPUS"
mkdir -p "$SCVAL_CORPUS" "$SPEC_CORPUS"

n=0
for f in "$RPC"/*.json; do
  xdr="$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); r=d.get("response",{}).get("result",{}); res=(r.get("results") or [{}])[0]; print(res.get("xdr","") if isinstance(res,dict) else "")' "$f")"
  [ -n "$xdr" ] || continue
  printf '%s' "$xdr" | base64 -d > "$SCVAL_CORPUS/seed_$(basename "$f" .json)" 2>/dev/null || continue
  n=$((n + 1))
done

m=0
for w in "$WASMS"/*.wasm; do
  cp "$w" "$SPEC_CORPUS/seed_$(basename "$w")"
  m=$((m + 1))
done

echo "scval_from_xdr corpus: $n seeds"
echo "parse_spec corpus:     $m seeds"
