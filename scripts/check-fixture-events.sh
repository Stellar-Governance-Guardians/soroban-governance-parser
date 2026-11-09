#!/usr/bin/env bash
# Live-tier check for the phase-1 fixture governor's events.
#
# The RPC only serves events newer than its oldest ledger (retention window).
# The phase-1 fixtures are fixed ledgers from 2026-10-05, so once that window
# rolls past them no RPC can return them again. Treating that as a red live
# check forever was wrong: it reported a repository defect where there was none.
#
# Outcomes (never silent, never a fake PASS):
#   PASS     exit 0  getEvents still returns the captured events.
#   EXPIRED  exit 0  the fixture ledger is older than the RPC retention floor;
#                    the committed capture is then the record. Prints both
#                    ledger numbers and the re-seed command.
#   FAIL     exit 1  the ledger IS inside retention but getEvents returns
#                    nothing, or the RPC answer is malformed/missing.
#
# Network-free self-test: bash scripts/check-fixture-events.sh --self-test
set -uo pipefail
cd "$(dirname "$0")/.."

RPC="${SGP_RPC_URL:-https://soroban-testnet.stellar.org}"
CONTRACT="$(jq -r '.contracts.testnet.fixtureGovernor.contractId' deployments.json)"
LEDGER="$(jq -r '.contracts.testnet.fixtureGovernor.fixtureEvents.propose.ledger' deployments.json)"

evaluate() { # $1 = getHealth JSON, $2 = getEvents JSON (optional when expired)
  local health="$1" events="${2:-}" oldest count
  oldest="$(jq -r '.result.oldestLedger // empty' <<<"$health" 2>/dev/null || true)"
  if ! [[ "${oldest:-}" =~ ^[0-9]+$ ]]; then
    echo "FAIL: getHealth returned no numeric result.oldestLedger" >&2
    return 1
  fi
  if [ "$LEDGER" -lt "$oldest" ]; then
    echo "EXPIRED: fixture ledger $LEDGER is older than the RPC retention floor $oldest."
    echo "The committed capture tests/fixtures/phase1-fixture-events.json is the record."
    echo "Re-seed to restore live coverage: bash scripts/prove-phase1.sh"
    return 0
  fi
  count="$(jq -r '(.result.events // []) | length' <<<"$events" 2>/dev/null || true)"
  if ! [[ "${count:-}" =~ ^[0-9]+$ ]]; then
    echo "FAIL: getEvents returned no result.events array" >&2
    return 1
  fi
  if [ "$count" -eq 0 ]; then
    echo "FAIL: contract $CONTRACT is inside retention (fixture ledger $LEDGER >= $oldest) but getEvents returned no events" >&2
    return 1
  fi
  echo "PASS: fixture events still queryable at ledger $LEDGER (oldest $oldest, $count events)"
  return 0
}

self_test() {
  local failures=0 out status
  LEDGER=100

  out="$(evaluate '{"result":{"oldestLedger":500}}' '' 2>&1)"
  status=$?
  if [ "$status" -eq 0 ] && printf '%s' "$out" | grep -q '^EXPIRED:'; then
    echo "ok   expired-fixture-is-explicit"
  else
    echo "FAIL expired-fixture-is-explicit (status=$status, out=$out)"; failures=$((failures+1))
  fi

  out="$(evaluate '{"result":{"oldestLedger":1}}' '{"result":{"events":[]}}' 2>&1)"
  status=$?
  if [ "$status" -ne 0 ] && printf '%s' "$out" | grep -q '^FAIL:'; then
    echo "ok   in-range-empty-events-fails"
  else
    echo "FAIL in-range-empty-events-fails (status=$status)"; failures=$((failures+1))
  fi

  out="$(evaluate '{"result":{"oldestLedger":1}}' '{"result":{"events":[{"ledger":100}]}}' 2>&1)"
  status=$?
  if [ "$status" -eq 0 ] && printf '%s' "$out" | grep -q '^PASS:'; then
    echo "ok   in-range-events-pass"
  else
    echo "FAIL in-range-events-pass (status=$status, out=$out)"; failures=$((failures+1))
  fi

  out="$(evaluate '{"result":{}}' '' 2>&1)"
  status=$?
  if [ "$status" -ne 0 ] && printf '%s' "$out" | grep -q '^FAIL:'; then
    echo "ok   missing-oldest-ledger-fails"
  else
    echo "FAIL missing-oldest-ledger-fails (status=$status)"; failures=$((failures+1))
  fi

  echo "fixture-events self-test: $((4 - failures)) passed, ${failures} failed"
  [ "$failures" -eq 0 ]
}

if [ "${1:-}" = "--self-test" ]; then
  self_test
  exit $?
fi

health="$(curl -sS --max-time 60 -X POST "$RPC" -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth","params":{}}')"
if ! printf '%s' "$health" | jq -e '.result' >/dev/null 2>&1; then
  echo "FAIL: getHealth failed: $health" >&2
  exit 1
fi

oldest="$(jq -r '.result.oldestLedger' <<<"$health")"
if [ "$LEDGER" -lt "$oldest" ]; then
  evaluate "$health"
  exit $?
fi

events="$(curl -sS --max-time 60 -X POST "$RPC" -H 'Content-Type: application/json' \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getEvents\",\"params\":{\"startLedger\":${LEDGER},\"filters\":[{\"type\":\"contract\",\"contractIds\":[\"${CONTRACT}\"]}],\"pagination\":{\"limit\":5}}}")"
if ! printf '%s' "$events" | jq -e '.result' >/dev/null 2>&1; then
  echo "FAIL: getEvents failed: $events" >&2
  exit 1
fi
evaluate "$health" "$events"
