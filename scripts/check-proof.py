#!/usr/bin/env python3
"""Verify the recorded phase-1 proof against the README/SPEC evidence claims.

Reads tests/fixtures/phase1-proof.json (produced by scripts/prove-phase1.sh
against live Stellar testnet) and fails loudly if any recorded fact does not
support the claim made in the docs. Offline: it checks the committed capture,
not the network.
"""

import json
import sys

PROOF = "tests/fixtures/phase1-proof.json"
EXPECTED_FUNCTIONS = {"propose", "vote", "get_proposal"}
MIN_TESTS = 23
RETENTION_WINDOW = 120960

failures = []


def check(label, ok, detail=""):
    if ok:
        print(f"  ok   {label}")
    else:
        print(f"  FAIL {label}{': ' + detail if detail else ''}")
        failures.append(label)


def main():
    try:
        with open(PROOF, encoding="utf-8") as fh:
            proof = json.load(fh)
    except (OSError, json.JSONDecodeError) as exc:
        print(f"FATAL: cannot read {PROOF}: {exc}", file=sys.stderr)
        print("Run scripts/prove-phase1.sh (needs live testnet) to regenerate.", file=sys.stderr)
        return 1

    print(f"checking {PROOF} (phase={proof.get('phase')}, rpc={proof.get('rpc')})")
    check("phase == 1", proof.get("phase") == 1, str(proof.get("phase")))
    check("rpc is the SDF public testnet endpoint",
          proof.get("rpc") == "https://soroban-testnet.stellar.org", str(proof.get("rpc")))

    health = proof.get("health", {}).get("result", {})
    check("getHealth status healthy", health.get("status") == "healthy", str(health.get("status")))
    check(f"ledgerRetentionWindow == {RETENTION_WINDOW}",
          health.get("ledgerRetentionWindow") == RETENTION_WINDOW,
          str(health.get("ledgerRetentionWindow")))

    fixture = proof.get("fixtureContract", {})
    contract_id = fixture.get("contractId", "")
    check("fixture contract id is a strkey C-address",
          contract_id.startswith("C") and len(contract_id) == 56, contract_id)

    captured = fixture.get("eventsCaptured") or []
    check("at least 2 live events captured", len(captured) >= 2, f"got {len(captured)}")
    for event in captured:
        check(f"captured event has 64-hex tx hash (ledger {event.get('ledger')})",
              len(str(event.get("txHash", ""))) == 64, str(event.get("txHash")))

    decoded = fixture.get("eventsDecoded") or []
    check("decoded events match captured count", len(decoded) == len(captured),
          f"{len(decoded)} vs {len(captured)}")
    flat = json.dumps(decoded)
    check("decoded topics include proposal_created", "proposal_created" in flat)
    check("decoded topics include vote_cast", "vote_cast" in flat)
    check("u128 payload preserved as decimal string", '"1000000"' in flat)

    spec_functions = {f.get("name") for f in fixture.get("liveSpec", {}).get("spec", {}).get("functions", [])}
    check("live contractspecv0 exposes propose/vote/get_proposal",
          EXPECTED_FUNCTIONS <= spec_functions, str(sorted(spec_functions)))
    source = fixture.get("liveSpec", {}).get("source", {})
    check("live spec records the RPC and ledger it was read at",
          bool(source.get("rpc")) and isinstance(source.get("atLedger"), int), json.dumps(source)[:160])

    negative = proof.get("negativeProof", {})
    check("negative proof: non-WASM contract exited non-zero",
          isinstance(negative.get("exitCode"), int) and negative["exitCode"] != 0,
          str(negative.get("exitCode")))
    check("negative proof: error names the missing contractspecv0",
          "contractspecv0" in str(negative.get("error", "")), str(negative.get("error"))[:160])

    toolchain = proof.get("toolchain", {})
    check(f"at least {MIN_TESTS} unit tests passed",
          isinstance(toolchain.get("unitTestsPassed"), int) and toolchain["unitTestsPassed"] >= MIN_TESTS,
          str(toolchain.get("unitTestsPassed")))
    check("clippy pedantic -D warnings clean", toolchain.get("clippy") == "clean",
          str(toolchain.get("clippy")))
    check("parser wasm sha256 recorded", len(str(toolchain.get("parserWasmSha256", ""))) == 64,
          str(toolchain.get("parserWasmSha256")))

    if failures:
        print(f"\n{len(failures)} claim(s) not supported by {PROOF}: {failures}", file=sys.stderr)
        return 1
    print("\nall recorded-evidence claims hold")
    return 0


if __name__ == "__main__":
    sys.exit(main())
