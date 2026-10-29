#!/usr/bin/env python3
"""Generate docs-site/src/generated/evidence.md from deployments.json.

Every contract in the registry is rendered with its contract id, deploy tx,
ledger and WASM hash, plus explorer links. The registry itself is checked
against committed fixtures by `scripts/check-deployments.py`; this page only
renders what that file says.

Usage: gen-evidence.py [--check]
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "deployments.json"
TARGET = ROOT / "docs-site/src/generated/evidence.md"
# Committed provenance the page points at; generation fails if either is gone.
PROOF = "tests/fixtures/phase1-proof.json"
FIXTURE_INDEX = "tests/fixtures/README.md"

EXPLORER = "https://stellar.expert/explorer"


def short(value: str, keep: int = 12) -> str:
    return value if len(value) <= keep else value[:keep] + "…"


def flatten(contracts: dict) -> list[tuple[str, dict]]:
    """Flatten the registry's one-level contract groups into (key, entry) rows."""
    rows: list[tuple[str, dict]] = []
    for key, value in contracts.items():
        if key.startswith("$") or not isinstance(value, dict):
            continue
        if "contractId" in value:
            rows.append((key, value))
            continue
        for sub_key, sub_value in value.items():
            if sub_key.startswith("$") or not isinstance(sub_value, dict):
                continue
            rows.append((f"{key}.{sub_key}", sub_value))
    return rows


def render(registry: dict, digest: str) -> str:
    out: list[str] = []
    out.append("# Evidence")
    out.append("")
    out.append(
        "Generated from `deployments.json` by `docs-site/scripts/gen-evidence.py` "
        f"(sha256 `{digest}`). Do not edit by hand. The registry is checked against "
        "the committed fixtures and upstream pins by `scripts/check-deployments.py` "
        "in the PR gate; this page renders it."
    )
    out.append("")
    out.append(
        "Every contract id below was deployed on live testnet with the recorded "
        "transaction hash, or captured from the public source named in "
        "`tests/fixtures/README.md`. There are no mock ids."
    )
    out.append("")

    networks = registry.get("networks", {})
    contracts_by_network = registry.get("contracts", {})

    for network, meta in networks.items():
        out.append(f"## Network: {network}")
        out.append("")
        out.append("| Field | Value |")
        out.append("|---|---|")
        out.append(f"| RPC | `{meta.get('rpcUrl', '')}` |")
        out.append(f"| Passphrase | `{meta.get('networkPassphrase', '')}` |")
        retention = meta.get("ledgerRetentionWindow")
        out.append(f"| RPC retention window | {retention:,} ledgers |" if isinstance(retention, int) else "| RPC retention window | not recorded |")
        out.append(f"| Retention verified | {meta.get('retentionWindowVerifiedAt', 'not recorded')} |")
        evidence = meta.get("retentionWindowEvidence")
        if evidence:
            out.append(f"| Retention evidence | `{evidence}` |")
        out.append("")

        rows = flatten(contracts_by_network.get(network, {}))
        out.append(f"### Contracts ({len(rows)})")
        out.append("")
        out.append("| Key | Contract id | Purpose | Deployed at ledger | Deploy tx |")
        out.append("|---|---|---|---|---|")
        for key, entry in rows:
            contract_id = entry.get("contractId", "")
            purpose = (entry.get("purpose") or entry.get("role") or "").replace("|", "\\|")
            ledger = entry.get("deployedAtLedger")
            ledger_text = f"{ledger:,}" if isinstance(ledger, int) else "not recorded"
            tx = entry.get("deployTxHash")
            tx_text = (
                f"[`{short(tx)}`]({EXPLORER}/{network}/tx/{tx})" if tx else "not recorded"
            )
            out.append(
                f"| `{key}` | [`{short(contract_id, 16)}`]({EXPLORER}/{network}/contract/{contract_id}) "
                f"| {purpose} | {ledger_text} | {tx_text} |"
            )
        out.append("")

        wasm_rows = [(key, entry) for key, entry in rows if entry.get("wasmSha256")]
        if wasm_rows:
            out.append("### WASM hashes")
            out.append("")
            out.append("| Key | WASM sha256 | Bytes | Upload tx |")
            out.append("|---|---|---|---|")
            for key, entry in wasm_rows:
                wasm_sha = entry["wasmSha256"]
                size = entry.get("wasmBytes")
                size_text = f"{size:,}" if isinstance(size, int) else "not recorded"
                upload = entry.get("wasmUploadTxHash")
                upload_text = (
                    f"[`{short(upload)}`]({EXPLORER}/{network}/tx/{upload})"
                    if upload
                    else "not recorded"
                )
                out.append(f"| `{key}` | `{wasm_sha}` | {size_text} | {upload_text} |")
            out.append("")

    if not (ROOT / PROOF).exists() or not (ROOT / FIXTURE_INDEX).exists():
        raise SystemExit(
            f"gen-evidence: missing provenance file ({PROOF} or {FIXTURE_INDEX})"
        )
    out.append("## Fixtures and proofs")
    out.append("")
    out.append(
        f"The raw RPC responses these contracts were deployed with are committed "
        f"under `tests/fixtures/`; `{FIXTURE_INDEX}` records the provenance of every "
        f"capture, and `{PROOF}` is the recorded live-testnet proof that the phase 1 "
        f"claims rest on. `python3 scripts/check-deployments.py` re-checks this "
        f"registry against those fixtures and the upstream pins in the PR gate."
    )
    out.append("")
    total = sum(len(flatten(c)) for c in contracts_by_network.values())
    out.append("## Totals")
    out.append("")
    out.append(f"- Registered contracts: **{total}**")
    out.append(f"- Networks: **{len(networks)}**")
    out.append("")
    out.append(
        "The contract ids above are testnet deployments. Testnet contracts can be "
        "removed, archived or lose their TTL entries; `sgp ttl --contract <id>` "
        "checks the live remaining ledgers and fails closed when entries are "
        "missing."
    )
    out.append("")
    return "\n".join(out)


def main() -> int:
    raw = SOURCE.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    content = render(json.loads(raw), digest)
    check = "--check" in sys.argv[1:]
    if check:
        current = TARGET.read_text(encoding="utf-8") if TARGET.exists() else ""
        if current != content:
            print(f"FAIL {TARGET.relative_to(ROOT)} is stale; run gen-evidence.py", file=sys.stderr)
            return 1
        print("ok evidence page is current")
        return 0
    TARGET.write_text(content, encoding="utf-8")
    print(f"wrote {TARGET.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
