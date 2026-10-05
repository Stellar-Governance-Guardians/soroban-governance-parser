#!/usr/bin/env bash
# Phase 1 proof: exercises the REAL parser pipeline against live Stellar
# testnet and writes tests/fixtures/phase1-proof.json.
#
# What it proves (each step fail-closed):
#  1. RPC health + retention window (getHealth)
#  2. Live event capture for the fixture governor (getEvents, small window)
#  3. contractspecv0 fetch+parse from the deployed WASM (getLedgerEntries)
#  4. ScVal decoding of real captured topic/value XDR
#  5. Negative proof: non-WASM (SAC) contract fails loudly, never silently
#  6. Toolchain state: tests, clippy, wasm artifact hash
#
# If the fixture contract was wiped by a testnet reset, redeploy first:
#   cd scripts/seed-testnet && stellar contract build
#   stellar keys generate fixture-governor --network testnet   # friendbot-funded
#   stellar contract deploy --wasm target/wasm32v1-none/release/sgg_fixture_governor.wasm \
#     --source fixture-governor --network testnet
#   # then invoke propose/vote and update deployments.json + tests/fixtures/README.md
set -euo pipefail
cd "$(dirname "$0")/.."

RPC="${RPC:-https://soroban-testnet.stellar.org}"
SGP="./target/release/sgp"
FIXTURES="tests/fixtures"
mkdir -p "$FIXTURES"

CONTRACT_ID=$(python3 -c "import json; print(json.load(open('deployments.json'))['contracts']['testnet']['fixtureGovernor']['contractId'])")
if [ "$CONTRACT_ID" = "None" ] || [ -z "$CONTRACT_ID" ]; then
  echo "FATAL: deployments.json has no fixture contract id. Deploy first (see header)." >&2
  exit 1
fi

echo "== building CLI =="
cargo build --release -p soroban-governance-cli 2>&1 | tail -30

echo "== 1. health =="
"$SGP" health --rpc "$RPC" | tee "$FIXTURES/phase1-rpc-getHealth.json"

echo "== 2. capture fixture events =="
# Window around the recorded deployment ledgers; small on purpose — the
# public RPC times out on wide ranges (verified 2026-10-05).
DEPLOY_LEDGER=$(python3 -c "import json; print(json.load(open('deployments.json'))['contracts']['testnet']['fixtureGovernor']['deployedAtLedger'])")
START=$((DEPLOY_LEDGER - 20)); END=$((DEPLOY_LEDGER + 40))
curl -sS --max-time 60 -X POST "$RPC" -H 'Content-Type: application/json' -d "{
  \"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getEvents\",
  \"params\":{\"startLedger\":$START,\"endLedger\":$END,
    \"filters\":[{\"type\":\"contract\",\"contractIds\":[\"$CONTRACT_ID\"]}]}}" \
  > "$FIXTURES/phase1-fixture-events.json"
python3 - <<'PYEOF'
import json, sys
d = json.load(open("tests/fixtures/phase1-fixture-events.json"))
evs = d.get("result", {}).get("events")
if evs is None:
    print("FATAL: getEvents returned no result:", json.dumps(d)[:400], file=sys.stderr)
    sys.exit(1)
if len(evs) < 2:
    print(f"FATAL: expected >=2 fixture events, got {len(evs)} — testnet reset?", file=sys.stderr)
    sys.exit(1)
print(f"captured {len(evs)} events")
PYEOF

echo "== 3. fetch+parse live contractspecv0 =="
"$SGP" fetch-spec --contract "$CONTRACT_ID" --rpc "$RPC" > /tmp/phase1-spec.json
python3 - <<'PYEOF'
import json, sys
spec = json.load(open("/tmp/phase1-spec.json"))
names = {f["name"] for f in spec["spec"]["functions"]}
if not {"propose", "vote", "get_proposal"} <= names:
    print("FATAL: expected functions missing from live spec:", names, file=sys.stderr)
    sys.exit(1)
print("live spec functions:", sorted(names))
PYEOF

echo "== 4. decode real captured XDR =="
python3 - <<'PYEOF' > /tmp/phase1-decoded.json
import json, subprocess, sys
evs = json.load(open("tests/fixtures/phase1-fixture-events.json"))["result"]["events"]
out = []
for e in evs:
    row = {"ledger": e["ledger"], "txHash": e["txHash"], "topics": [], "value": None}
    for t in e["topic"]:
        r = subprocess.run(["./target/release/sgp", "decode-scval", t],
                           capture_output=True, text=True)
        if r.returncode != 0:
            print("FATAL: topic decode failed:", r.stderr, file=sys.stderr); sys.exit(1)
        row["topics"].append(json.loads(r.stdout))
    r = subprocess.run(["./target/release/sgp", "decode-scval", e["value"]],
                       capture_output=True, text=True)
    if r.returncode != 0:
        print("FATAL: value decode failed:", r.stderr, file=sys.stderr); sys.exit(1)
    row["value"] = json.loads(r.stdout)
    out.append(row)
json.dump(out, sys.stdout, indent=2)
PYEOF
cat /tmp/phase1-decoded.json | head -20

echo "== 5. negative proof: SAC has no contractspecv0 =="
set +e
SAC_ERR=$("$SGP" fetch-spec --contract CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC --rpc "$RPC" 2>&1)
SAC_EXIT=$?
set -e
if [ "$SAC_EXIT" -eq 0 ]; then
  echo "FATAL: expected failure on non-WASM contract, got success" >&2
  exit 1
fi
echo "failed as required (exit $SAC_EXIT): $SAC_ERR"

echo "== 6. toolchain state =="
TEST_COUNT=$(cargo test --workspace 2>/dev/null | grep -oE 'ok\. [0-9]+ passed' | grep -oE '[0-9]+' | sort -rn | head -1)
if [ -z "$TEST_COUNT" ] || [ "$TEST_COUNT" -eq 0 ]; then
  echo "FATAL: no passing unit tests detected" >&2
  exit 1
fi
cargo clippy --workspace --all-targets -- -D warnings >/dev/null 2>&1 && CLIPPY="clean" || CLIPPY="FAILED"
if [ "$CLIPPY" = "FAILED" ]; then
  echo "FATAL: clippy pedantic -D warnings is not clean; re-run without redirection to see lints" >&2
  exit 1
fi
cargo build -p soroban-governance-wasm --target wasm32-unknown-unknown --release 2>&1 | tail -20
PARSER_WASM_SHA=$(sha256sum target/wasm32-unknown-unknown/release/soroban_governance_wasm.wasm | cut -d' ' -f1)

echo "== assembling proof =="
python3 - <<PYEOF
import json, hashlib
proof = {
  "phase": 1,
  "generatedAtUtc": __import__("datetime").datetime.now(__import__("datetime").timezone.utc).isoformat(),
  "rpc": "$RPC",
  "health": json.load(open("tests/fixtures/phase1-rpc-getHealth.json")),
  "fixtureContract": {
    "contractId": "$CONTRACT_ID",
    "deployedAtLedger": $DEPLOY_LEDGER,
    "eventsCaptured": json.load(open("tests/fixtures/phase1-fixture-events.json"))["result"]["events"],
    "eventsDecoded": json.load(open("/tmp/phase1-decoded.json")),
    "liveSpec": json.load(open("/tmp/phase1-spec.json")),
  },
  "negativeProof": {
    "contractId": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
    "expectedFailure": True,
    "exitCode": $SAC_EXIT,
    "error": """$SAC_ERR""".strip(),
  },
  "toolchain": {
    "unitTestsPassed": int("$TEST_COUNT"),
    "clippy": "$CLIPPY",
    "parserWasmSha256": "$PARSER_WASM_SHA",
  },
}
json.dump(proof, open("tests/fixtures/phase1-proof.json", "w"), indent=2)
print("wrote tests/fixtures/phase1-proof.json")
PYEOF
echo "PHASE 1 PROOF OK"
