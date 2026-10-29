#!/usr/bin/env python3
"""Machine-check the claims this documentation site makes (charter rule 7).

Every check is offline and deterministic. Generated pages are regenerated with
--check so a page cannot disagree with its source file; versions, hashes, counts
and the adapter list are read from the committed sources, never restated here.

Usage: check-docs-claims.py
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

SITE = Path(__file__).resolve().parents[1]
ROOT = SITE.parent
SRC = SITE / "src"
SCRIPTS = SITE / "scripts"

results: list[tuple[bool, str, str]] = []


def check(ok: bool, name: str, detail: str) -> None:
    results.append((ok, name, detail))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(rel: str, base: Path = ROOT) -> str:
    return (base / rel).read_text(encoding="utf-8")


def main() -> int:
    # 1. Generated pages match their sources.
    for script, args in [
        ("gen-risk-rules.py", []),
        ("gen-cli.py", []),
        ("gen-api.py", []),
        ("gen-evidence.py", []),
        ("gen-upstream.py", []),
    ]:
        proc = subprocess.run(
            [sys.executable, str(SCRIPTS / script), *args, "--check"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        check(
            proc.returncode == 0,
            f"generated:{script}",
            (proc.stdout.strip() or proc.stderr.strip()).splitlines()[-1]
            if (proc.stdout.strip() or proc.stderr.strip())
            else "no output",
        )

    # 2. sources.lock: vendored copies hash exactly as pinned; org namespace only.
    lock = json.loads((SITE / "sources.lock").read_text(encoding="utf-8"))
    org = "https://github.com/Stellar-Governance-Guardians/"
    for entry in lock.get("sources", []):
        name = entry.get("repo", "?").rsplit("/", 1)[-1]
        if not entry.get("repo", "").startswith(org):
            check(False, f"pin:{name}", f"repo is not in the organization namespace: {entry.get('repo')}")
            continue
        if not re.fullmatch(r"[0-9a-f]{40}", entry.get("commitSha", "")):
            check(False, f"pin:{name}", "commitSha is not a full 40-character SHA")
            continue
        vendored = SITE / "pinned" / name / entry["path"]
        if not vendored.exists():
            check(False, f"pin:{name}", f"vendored copy missing at {vendored.relative_to(SITE)}")
            continue
        actual = sha256(vendored)
        check(
            actual == entry.get("sha256"),
            f"pin:{name}",
            f"{entry['path']} sha256 {actual}",
        )

    # 3. Parser WASM version + sha256 shown on the site equal the release lock.
    release = json.loads(read("releases/parser-wasm.lock.json"))
    wasm_page = read("docs-site/src/parser/wasm-usage.md")
    check(
        release["version"] in wasm_page and release["sha256"] in wasm_page,
        "wasm-release-pin",
        f"{release['version']} / {release['sha256'][:12]}… shown on WASM usage",
    )

    # 4. Risk rule count on the generated page equals the source table.
    risk_source = read("crates/core/src/risk.rs")
    risk_block = risk_source[risk_source.index("pub static RULES") : risk_source.index("];", risk_source.index("pub static RULES"))]
    rules = re.findall(
        r'\(\s*"([^"]+)"\s*,\s*RiskTier::([A-Za-z]+)\s*,\s*"((?:[^"\\]|\\.)*)"\s*,?\s*\)',
        risk_block,
        re.DOTALL,
    )
    risk_page = read("docs-site/src/generated/risk-rules.md")
    check(
        f"The table holds **{len(rules)} rules**" in risk_page,
        "risk-rule-count",
        f"{len(rules)} rules parsed from risk.rs",
    )

    # 5. SDL sha256 shown on the API page equals the committed SDL.
    sdl_sha = sha256(ROOT / "schemas/governance-v1.graphql")
    api_page = read("docs-site/src/generated/api.md")
    check(sdl_sha in api_page, "sdl-sha256", f"schemas/governance-v1.graphql {sdl_sha[:12]}…")

    # 6. Deployment count on the evidence page equals the registry.
    registry = json.loads(read("deployments.json"))

    def flatten(contracts: dict) -> list:
        rows = []
        for key, value in contracts.items():
            if key.startswith("$") or not isinstance(value, dict):
                continue
            if "contractId" in value:
                rows.append(value)
            else:
                rows.extend(
                    sub
                    for sub_key, sub in value.items()
                    if not sub_key.startswith("$") and isinstance(sub, dict)
                )
        return rows

    total = sum(len(flatten(c)) for c in registry.get("contracts", {}).values())
    evidence_page = read("docs-site/src/generated/evidence.md")
    check(
        f"Registered contracts: **{total}**" in evidence_page
        and f"### Contracts ({total})" in evidence_page,
        "deployment-count",
        f"{total} registered contracts rendered",
    )

    # 7. Adapter list: Script3 implemented, OpenZeppelin not, matching claims.json.
    claims = json.loads(read("claims.json"))["claims"]
    oz_claim = next((c for c in claims if c["id"] == "oz-adapter-not-implemented"), None)
    adapter_page = read("docs-site/src/parser/adapter-model.md")
    oz_page = read("docs-site/src/parser/openzeppelin-adapter.md")
    check(
        oz_claim is not None
        and "not implemented" in adapter_page
        and "Status: planned" in oz_page
        and "| Script3 (`script3-soroban-governor`) | implemented |" in adapter_page,
        "adapter-list",
        "Script3 implemented / OpenZeppelin planned, consistent with claims.json",
    )

    # 8. No external script, font or tracker in the book configuration.
    book_toml = read("docs-site/book.toml")
    external_js = re.findall(r'additional-js\s*=\s*\[(.*?)\]', book_toml, re.DOTALL)
    js_entries = re.findall(r'"([^"]+)"', external_js[0]) if external_js else []
    check(
        all(not entry.startswith(("http://", "https://", "//")) for entry in js_entries),
        "no-external-assets",
        f"additional-js: {', '.join(js_entries) or 'none'}",
    )

    # 9. Pages the deployment check requires must exist and be listed in SUMMARY.
    required = {
        "index.md": "home",
        "architecture.md": "architecture",
        "generated/api.md": "api reference",
        "generated/evidence.md": "evidence",
    }
    summary = read("docs-site/src/SUMMARY.md")
    missing = [
        name
        for path, name in required.items()
        if not (SRC / path).exists() or Path(path).name not in summary
    ]
    check(not missing, "required-pages", "home, architecture, API reference, evidence present" if not missing else f"missing: {missing}")

    # 10. Marketing wording the charter forbids, in hand-written prose.
    # Hyphenated technical compounds (read-only, additive-only) are allowed; a
    # bare "first"/"only" claim is not. Generated pages quote sources (SDL
    # descriptions, CLI help) and are exempt.
    forbidden = re.compile(r"(?<![\w-])(first|only)(?![\w-])", re.IGNORECASE)
    offenders: list[str] = []
    for page in sorted(SRC.rglob("*.md")):
        if page.parent.name == "generated":
            continue
        for number, line in enumerate(page.read_text(encoding="utf-8").splitlines(), 1):
            if forbidden.search(line):
                offenders.append(f"{page.relative_to(SITE)}:{number}: {line.strip()[:80]}")
    check(
        not offenders,
        "no-first-or-only",
        "no 'first'/'only' wording in hand-written pages" if not offenders else "; ".join(offenders[:5]),
    )

    passed = sum(1 for ok, _n, _d in results if ok)
    for ok, name, detail in results:
        print(f"{'PASS' if ok else 'FAIL'}  {name}: {detail}")
    print(f"docs claims: {passed} passed, {len(results) - passed} failed")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
