#!/usr/bin/env bash
# Claims ledger checker (charter rule 8). Runs every claim in claims.json and
# reports PASS/FAIL. Exit non-zero on any FAIL. Online claims are skipped when
# OFFLINE=1 (and reported as SKIP, never as PASS).
set -u
cd "$(dirname "$0")/.."

OFFLINE="${OFFLINE:-0}"
python3 - "$OFFLINE" <<'PYEOF' > /tmp/claims-generated.sh
import json, sys
offline = sys.argv[1] == "1"
claims = json.load(open("claims.json"))["claims"]
print("set -u")
print("FAILED=0; PASSED=0; SKIPPED=0")
for c in claims:
    cmd = c["check"]["cmd"].replace('"', '\\"')
    if c.get("online") and offline:
        print(f'echo "SKIP  {c["id"]} (online claim, OFFLINE=1)"; SKIPPED=$((SKIPPED+1))')
    else:
        print(f'if bash -c "{cmd}" >/dev/null 2>&1; then echo "PASS  {c["id"]}"; PASSED=$((PASSED+1)); else echo "FAIL  {c["id"]}: {c["claim"]}"; FAILED=$((FAILED+1)); fi')
print('echo "claims: $PASSED passed, $FAILED failed, $SKIPPED skipped"')
print('test "$FAILED" -eq 0')
PYEOF

bash /tmp/claims-generated.sh
status=$?
rm -f /tmp/claims-generated.sh
exit $status
