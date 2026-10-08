#!/usr/bin/env python3
"""Offline claim check: deployments.json must agree with the committed evidence.

Enforces, with no network and no secrets:

  1. every registered seed-v2 contract's deployTxHash appears in
     tests/fixtures/seed-v2/index.json with status SUCCESS, and its ledger in
     deployments.json matches the ledger recorded for that fixture;
  2. the registered wasmSha256 matches BOTH the sha256 of the committed wasm
     file on disk AND the pinned sha256 in scripts/upstream/upstream.lock.json;
  3. the registered upstreamRepo / upstreamCommitSha match upstream.lock.json
     exactly, so the registry cannot drift from the pin;
  4. the registered wasmFile is the file upstream.lock.json records for that
     artifact, and its byte length matches;
  5. deployments.json contains no secret-looking material (private keys,
     seed phrases, mnemonics) -- charter rule 5.

Why this exists: deployments.json is the registry other repos (the indexer and
the dashboard, via the cross-repo check) treat as public truth. A stale or
hand-edited entry would make them ingest or display a contract id that no
fixture backs. Exit 0 = registry and evidence agree.
"""
import hashlib
import json
import pathlib
import re
import sys

root = pathlib.Path(__file__).resolve().parent.parent
dep = json.loads((root / "deployments.json").read_text())
lock = json.loads((root / "scripts" / "upstream" / "upstream.lock.json").read_text())
index = json.loads((root / "tests" / "fixtures" / "seed-v2" / "index.json").read_text())
phase1 = json.loads((root / "tests" / "fixtures" / "phase1-proof.json").read_text())

failures: list[str] = []
checked = 0


def fail(msg: str) -> None:
    failures.append(msg)
    print(f"FAIL {msg}")


# --- upstream.lock.json indexes -------------------------------------------------
gov_by_sha = {g["commitSha"]: g for g in lock["governors"]}
artifact_by_sha = {}
for g in lock["governors"]:
    for a in g["artifacts"]:
        # sha256 -> (governor, artifact). Two artifacts share a wasm in the seed
        # run (the Script3 mock subcall is reused for the OZ side); the map is
        # therefore keyed by (sha256, file) where it matters.
        artifact_by_sha.setdefault(a["sha256"], []).append((g, a))

# --- committed evidence ---------------------------------------------------------
fixture_by_hash: dict[str, dict] = {}
for f in index["fixtures"]:
    if f.get("txHash"):
        fixture_by_hash.setdefault(f["txHash"], f)


# --- checks ---------------------------------------------------------------------
# Group `fixtureGovernor` is the phase 1 proof contract: its evidence lives in
# tests/fixtures/phase1-proof.json (it has no upstream pin and its wasm is built
# from scripts/seed-testnet, not from a pinned upstream artifact). The two
# seedV2* groups carry the full upstream pin and are checked strictly below.
SEED_V2_GROUPS = ("seedV2Script3", "seedV2OpenZeppelin")


def is_contract_record(rec) -> bool:
    return isinstance(rec, dict) and "contractId" in rec


fixture_groups: list[tuple[str, str, dict]] = []
seed_groups: list[tuple[str, str, dict]] = []

# deployments.json is contracts.<network>.<name>, where <name> is either a single
# contract record or a named group of them. Walk both levels so nothing is
# silently skipped.
for network, entries in dep.get("contracts", {}).items():
    if not isinstance(entries, dict) or network.startswith("$"):
        continue
    for name, rec in entries.items():
        if name.startswith("$") or not isinstance(rec, dict):
            continue
        if is_contract_record(rec):
            (seed_groups if name in SEED_V2_GROUPS else fixture_groups).append((network, name, rec))
            continue
        for sub_name, sub_rec in rec.items():
            if sub_name.startswith("$") or not is_contract_record(sub_rec):
                continue
            (seed_groups if name in SEED_V2_GROUPS else fixture_groups).append(
                (f"{network}.{name}", sub_name, sub_rec)
            )

# --- phase 1 fixture governor: id, deploy ledger and event txs vs phase1-proof ---
p1 = phase1["fixtureContract"]
for group, name, rec in fixture_groups:
    checked += 1
    label = f"{group}.{name}"
    if rec["contractId"] != p1["contractId"]:
        fail(f"{label}: contractId {rec['contractId']} != phase1-proof {p1['contractId']}")
    if rec.get("deployedAtLedger") != p1["deployedAtLedger"]:
        fail(f"{label}: deployedAtLedger {rec.get('deployedAtLedger')} != phase1-proof {p1['deployedAtLedger']}")
    captured = {e.get("txHash") for e in p1.get("eventsCaptured", [])}
    for ev_name, ev in (rec.get("fixtureEvents") or {}).items():
        if ev.get("txHash") not in captured:
            fail(f"{label}: fixtureEvents.{ev_name} tx {ev.get('txHash')} not in phase1-proof eventsCaptured")
    if not re.fullmatch(r"C[A-Z2-7]{55}", rec["contractId"]):
        fail(f"{label}: contractId {rec['contractId']!r} is not a valid Soroban contract id")
    print(f"ok {label} (phase 1 evidence)")

for group, name, rec in seed_groups:
    checked += 1
    label = f"{group}.{name}"

    # (1) deploy tx backed by a committed fixture, and it succeeded
    tx = rec.get("deployTxHash")
    if not tx:
        fail(f"{label}: no deployTxHash")
        continue
    if not re.fullmatch(r"[0-9a-f]{64}", tx):
        fail(f"{label}: deployTxHash is not 64 lowercase hex chars")
    fixture = fixture_by_hash.get(tx)
    if fixture is None:
        fail(f"{label}: deployTxHash {tx} has no committed fixture")
        continue
    if fixture.get("status") != "SUCCESS":
        fail(f"{label}: deploy fixture {fixture['file']} is {fixture.get('status')}, not SUCCESS")
    if rec.get("deployedAtLedger") != fixture.get("ledger"):
        fail(
            f"{label}: deployedAtLedger {rec.get('deployedAtLedger')} != "
            f"fixture ledger {fixture.get('ledger')} ({fixture['file']})"
        )

    # (2)/(3)/(4) the wasm + upstream pin
    sha = rec.get("wasmSha256")
    entries = artifact_by_sha.get(sha, [])
    if not entries:
        fail(f"{label}: wasmSha256 {sha} is not pinned in upstream.lock.json")
        continue
    matching = [(g, a) for g, a in entries if rec.get("wasmFile", "").endswith(a["file"])]
    if not matching:
        names = sorted({a["file"] for _, a in entries})
        fail(f"{label}: wasmFile {rec.get('wasmFile')} does not match pinned artifact(s) {names}")
        continue
    governor, artifact = matching[0]

    on_disk = root / rec["wasmFile"]
    if not on_disk.exists():
        fail(f"{label}: wasmFile {rec['wasmFile']} is not committed")
    else:
        actual = hashlib.sha256(on_disk.read_bytes()).hexdigest()
        if actual != sha:
            fail(f"{label}: {rec['wasmFile']} hashes {actual}, registry says {sha}")
        if rec.get("wasmBytes") is not None and on_disk.stat().st_size != rec["wasmBytes"]:
            fail(
                f"{label}: {rec['wasmFile']} is {on_disk.stat().st_size} bytes, "
                f"registry says {rec['wasmBytes']}"
            )

    if rec.get("upstreamCommitSha") != governor["commitSha"]:
        fail(f"{label}: upstreamCommitSha {rec.get('upstreamCommitSha')} != pinned {governor['commitSha']}")
    if rec.get("upstreamRepo") != governor["repo"]:
        fail(f"{label}: upstreamRepo {rec.get('upstreamRepo')} != pinned {governor['repo']}")

    # contract id must look like a real Soroban contract id (C + 55 base32 chars)
    cid = rec.get("contractId", "")
    if not re.fullmatch(r"C[A-Z2-7]{55}", cid):
        fail(f"{label}: contractId {cid!r} is not a valid Soroban contract id")

    print(f"ok {label} (seed-v2 fixture {fixture['file']})")

print(f"registered contracts checked: {checked}")

# --- (5) no secret material in the registry -------------------------------------
SECRET_PATTERNS = [
    (r"(?i)\bprivate[_-]?key\b", "private key"),
    (r"(?i)\bseed[_-]?phrase\b", "seed phrase"),
    (r"(?i)\bsecret[_-]?key\b", "secret key"),
    (r"(?i)\bpassphrase\s*[=:]\s*\S", "passphrase assignment"),
    (r"(?i)\bapi[_-]?key\b", "api key"),
    (r"(?i)\bBEGIN [A-Z ]*PRIVATE KEY\b", "PEM private key"),
]
text = (root / "deployments.json").read_text()
# `networkPassphrase` is the PUBLIC Soroban network id and is required to read
# the chain; allow exactly that key and nothing else passphrase-shaped.
scan = text.replace('"networkPassphrase"', '"__public_network_id__"')
for pattern, what in SECRET_PATTERNS:
    m = re.search(pattern, scan)
    if m:
        fail(f"deployments.json contains {what}-shaped material: {m.group(0)!r}")

if failures:
    print(f"\ndeployments registry check: {len(failures)} failure(s)")
    sys.exit(1)

print("deployments registry check: OK (registry matches committed fixtures + upstream pins)")
