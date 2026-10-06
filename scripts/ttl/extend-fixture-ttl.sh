#!/usr/bin/env bash
# Idempotent fixture-contract TTL extension (charter process rule 5).
#
# Reads the fixture governor's instance/code TTL via `sgp ttl` and skips
# (exit 0, no tx) when remaining ledgers are at/above the threshold. Otherwise
# extends the entries with the testnet maximum minus one:
#   extendTo = 3110399 = maxEntryTTL (3110400) - 1
# stellar-core rejects extendTo == maxEntryTTL as ExtendFootprintTtl(Malformed)
# (verified on-chain 2026-10-06; see deployments.json fixtureGovernor.ttl).
#
# Usage: extend-fixture-ttl.sh [contractId] [sourceIdentity] [minRemaining]
#   contractId      default: fixtureGovernor.contractId from deployments.json
#   sourceIdentity  stellar CLI keystore identity holding XLM (never committed)
#   minRemaining    skip when both entries have >= this many ledgers left
set -euo pipefail
cd "$(dirname "$0")/../.."

CONTRACT_ID="${1:-$(jq -r '.contracts.testnet.fixtureGovernor.contractId' deployments.json)}"
SOURCE_IDENTITY="${2:-fixture-governor}"
MIN_REMAINING="${3:-1000000}"
EXTEND_BY=3110399
NETWORK=testnet
WASM="scripts/seed-testnet/target/wasm32v1-none/release/sgg_fixture_governor.wasm"

if [ ! -x target/debug/sgp ]; then
    cargo build -p soroban-governance-cli
fi
if [ ! -f "$WASM" ]; then
    (cd scripts/seed-testnet && stellar contract build)
fi

ttl_json=$(./target/debug/sgp ttl --contract "$CONTRACT_ID")
echo "$ttl_json" | jq .

inst_remaining=$(echo "$ttl_json" | jq -r '.instance.remainingLedgers')
code_remaining=$(echo "$ttl_json" | jq -r '.code.remainingLedgers')

if [ "$inst_remaining" -ge "$MIN_REMAINING" ] && [ "$code_remaining" -ge "$MIN_REMAINING" ]; then
    echo "SKIP: instance and code TTL remaining >= $MIN_REMAINING ledgers (no-op)"
    exit 0
fi

if [ "$inst_remaining" -lt "$MIN_REMAINING" ]; then
    echo "extending instance TTL by $EXTEND_BY ledgers"
    stellar contract extend --id "$CONTRACT_ID" --ledgers-to-extend "$EXTEND_BY" \
        --source-account "$SOURCE_IDENTITY" --network "$NETWORK"
fi

if [ "$code_remaining" -lt "$MIN_REMAINING" ]; then
    echo "extending code TTL by $EXTEND_BY ledgers"
    stellar contract extend --wasm "$WASM" --ledgers-to-extend "$EXTEND_BY" \
        --source-account "$SOURCE_IDENTITY" --network "$NETWORK"
fi

echo "post-extension TTL:"
./target/debug/sgp ttl --contract "$CONTRACT_ID" | jq .
echo "Record the printed tx hashes in deployments.json (fixtureGovernor.ttl)."
