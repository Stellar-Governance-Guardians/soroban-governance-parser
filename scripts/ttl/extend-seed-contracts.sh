#!/usr/bin/env bash
# Idempotent TTL extension for ALL seed-v2 fixture contracts (charter process
# rule 5). Reads every contract id from the gitignored .seed/state.json, checks
# its instance + code TTL with `sgp ttl`, and extends only the entries that are
# below the threshold. No-op (exit 0) when everything is healthy.
#
# extendTo = 3110399 = testnet maxEntryTTL (3110400) - 1; stellar-core rejects
# extendTo == maxEntryTTL (verified on-chain 2026-10-06, see deployments.json).
#
# Requires the seed working area (it holds the throwaway deployer key under
# .seed/keys, never committed). Run from the repo root.
#
# Usage: scripts/ttl/extend-seed-contracts.sh [minRemaining]
set -euo pipefail
cd "$(dirname "$0")/../.."

STATE=.seed/state.json
KEYS_DIR=.seed/keys
SOURCE=sgg-deployer
MIN_REMAINING="${1:-1000000}"
EXTEND_BY=3110399

if [ ! -f "$STATE" ]; then
    echo "FATAL: no $STATE — run scripts/seed-v2/deploy.js first (fail closed)" >&2
    exit 1
fi
if [ ! -x target/debug/sgp ]; then
    cargo build -p soroban-governance-cli
fi

while IFS=$'\t' read -r name id wasm_file; do
    ttl=$(./target/debug/sgp ttl --contract "$id")
    inst=$(echo "$ttl" | jq -r '.instance.remainingLedgers')
    code=$(echo "$ttl" | jq -r '.code.remainingLedgers')

    if [ "$inst" -ge "$MIN_REMAINING" ] && [ "$code" -ge "$MIN_REMAINING" ]; then
        echo "SKIP $name ($id): TTL healthy (instance=$inst code=$code)"
        continue
    fi

    if [ "$inst" -lt "$MIN_REMAINING" ]; then
        echo "extending $name instance by $EXTEND_BY ledgers"
        stellar contract extend --id "$id" --ledgers-to-extend "$EXTEND_BY" \
            --source-account "$SOURCE" --config-dir "$KEYS_DIR" --network testnet
    fi
    if [ "$code" -lt "$MIN_REMAINING" ] && [ -f "$wasm_file" ]; then
        echo "extending $name code by $EXTEND_BY ledgers ($wasm_file)"
        stellar contract extend --wasm "$wasm_file" --ledgers-to-extend "$EXTEND_BY" \
            --source-account "$SOURCE" --config-dir "$KEYS_DIR" --network testnet
    fi
    echo "  now: $(./target/debug/sgp ttl --contract "$id" | jq -c '{instance:.instance.remainingLedgers,code:.code.remainingLedgers}')"
done < <(jq -r --arg dir "tests/fixtures/seed-v2/wasms" \
    '.contracts | to_entries[] | [.key, .value.contractId, ($dir + "/" + (.value.wasmFile // ""))] | @tsv' "$STATE")

echo "SEED TTL OK"
