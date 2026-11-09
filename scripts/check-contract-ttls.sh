#!/usr/bin/env bash
# Live-tier TTL check for every contract registered in deployments.json.
#
# Three outcomes, all explicit:
#   ok       the instance and code entries have at least TTL_MIN_REMAINING.
#   EXPIRED  the contract still answers, but its instance or code entry has
#            liveUntilLedger 0. The seed-v2 deployments from 2026-10-08 have
#            aged out; refreshing them needs the funded deployer key, so this
#            is a ::warning:: with the re-seed command, not a repository defect.
#   FAIL     anything else (RPC error, unreadable answer, unknown contract).
#            A check that cannot read the chain must never look green.
#
# Exit 1 only when a contract is unreadable or the registry is empty.
#
# Offline self-test: bash scripts/check-contract-ttls.sh --self-test
set -uo pipefail
cd "$(dirname "$0")/.."

SGP="${SGP_BIN:-./target/debug/sgp}"
MIN="${TTL_MIN_REMAINING:-50000}"

run_check() {
  local contracts="$1" total=0 ok=0 expired=0 bad=0 contract out rc err
  for contract in $contracts; do
    total=$((total + 1))
    err="$(mktemp)"
    out="$("$SGP" ttl --contract "$contract" --min-remaining "$MIN" 2>"$err")"
    rc=$?
    if [ "$rc" -eq 0 ]; then
      ok=$((ok + 1))
      echo "ok $contract ($MIN+ ledgers remaining)"
    elif printf '%s' "$out" | jq -e '(.instance.liveUntilLedger // -1) == 0 or (.code.liveUntilLedger // -1) == 0' >/dev/null 2>&1; then
      expired=$((expired + 1))
      echo "::warning::expired fixture contract $contract (instance or code liveUntilLedger is 0); re-seed with bash scripts/seed-v2/deploy.js and extend with bash scripts/ttl/extend-fixture-ttl.sh using the funded deployer key"
    else
      bad=$((bad + 1))
      echo "::error::unhealthy contract $contract: $(tr '\n' ' ' < "$err")"
    fi
    rm -f "$err"
  done

  echo "ttl: checked $total contracts: $ok ok, $expired expired (warning), $bad failed"
  if [ "$total" -eq 0 ]; then
    echo "::error::the deployment registry listed no contracts; a check that checks nothing is a failure" >&2
    return 1
  fi
  if [ "$bad" -gt 0 ]; then
    return 1
  fi
  return 0
}

self_test() {
  local stub failures=0 out rc
  stub="$(mktemp)"
  cat > "$stub" <<'EOS'
#!/usr/bin/env bash
# args: ttl --contract <id> --min-remaining <n>
case "${3:-}" in
  GOOD*) echo '{"instance":{"liveUntilLedger":100},"code":{"liveUntilLedger":100}}'; exit 0 ;;
  EXPIRED*) echo '{"instance":{"liveUntilLedger":0},"code":{"liveUntilLedger":0}}'; echo 'Error: "TTL below minimum remaining 1 ledgers: instance, code"' >&2; exit 1 ;;
  *) echo 'Error: "unexpected RPC response shape: contract not found"' >&2; exit 1 ;;
esac
EOS
  chmod +x "$stub"

  out="$(CONTRACTS_OVERRIDE="GOOD1 EXPIRED1" SGP_BIN="$stub" bash "$0" 2>&1)"
  rc=$?
  if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -q '::warning::expired fixture contract EXPIRED1'; then
    echo "ok   expired-contract-warns-not-fails"
  else
    echo "FAIL expired-contract-warns-not-fails (rc=$rc)"; failures=$((failures + 1))
  fi

  out="$(CONTRACTS_OVERRIDE="GOOD1 BROKEN1" SGP_BIN="$stub" bash "$0" 2>&1)"
  rc=$?
  if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -q '::error::unhealthy contract BROKEN1'; then
    echo "ok   unreadable-contract-fails"
  else
    echo "FAIL unreadable-contract-fails (rc=$rc)"; failures=$((failures + 1))
  fi

  out="$(CONTRACTS_OVERRIDE="" SGP_BIN="$stub" bash "$0" 2>&1)"
  rc=$?
  if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -q 'listed no contracts'; then
    echo "ok   empty-registry-fails"
  else
    echo "FAIL empty-registry-fails (rc=$rc)"; failures=$((failures + 1))
  fi

  rm -f "$stub"
  echo "ttl self-test: $((3 - failures)) passed, ${failures} failed"
  [ "$failures" -eq 0 ]
}

if [ "${1:-}" = "--self-test" ]; then
  self_test
  exit $?
fi

if [ -n "${CONTRACTS_OVERRIDE+set}" ]; then
  contracts="$CONTRACTS_OVERRIDE"
else
  contracts="$(bash scripts/list-registered-contracts.sh)"
fi
run_check "$contracts"
