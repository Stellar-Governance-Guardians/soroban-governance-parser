#!/usr/bin/env bash
# Print every contract id registered in deployments.json, one per line, deduplicated.
#
# deployments.json nests contracts as contracts.<network>.<name>, where <name> is
# either a single contract record (has contractId) or a named group of them (the
# seedV2Script3 / seedV2OpenZeppelin groups). This descends both levels, so a
# contract added to a group is picked up automatically -- a flat jq that only
# reads contractId at the top level silently skips every grouped contract, which
# would make the live TTL check look green while checking almost nothing.
#
# Public data only; reads no secrets and makes no network calls.
set -euo pipefail
cd "$(dirname "$0")/.."
jq -r '
  .contracts
  | to_entries[]
  | select(.key | startswith("$") | not)
  | .value
  | to_entries[]
  | select(.key | startswith("$") | not)
  | .value as $v
  | if ($v.contractId? // null) != null then $v.contractId
    else ($v
          | to_entries[]
          | select(.key | startswith("$") | not)
          | .value.contractId? // empty)
    end
' deployments.json | sort -u | grep -E '^C[A-Z2-7]{55}$' || true
