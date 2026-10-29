#!/usr/bin/env python3
"""Generate docs-site/src/generated/risk-rules.md from crates/core/src/risk.rs.

The page is generated, never hand-edited: the rule table is the source of truth
and must be reproduced exactly. CI regenerates it and fails on any diff.

Usage: gen-risk-rules.py [--check]
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "crates/core/src/risk.rs"
TARGET = ROOT / "docs-site/src/generated/risk-rules.md"

TIER_ORDER = ["Critical", "High", "Medium", "Low"]
TIER_NOTE = {
    "Critical": "Rules that can move value or change contract code/ownership.",
    "High": "Rules that change governance parameters or state.",
    "Medium": "Rules with observable but bounded effects.",
    "Low": "Read-only or informational actions.",
}

ENTRY = re.compile(
    r'\(\s*"([^"]+)"\s*,\s*RiskTier::([A-Za-z]+)\s*,\s*"((?:[^"\\]|\\.)*)"\s*,?\s*\)',
    re.DOTALL,
)


def read_rules() -> list[tuple[str, str, str]]:
    text = SOURCE.read_text(encoding="utf-8")
    start = text.index("pub static RULES")
    end = text.index("];", start)
    block = text[start:end]
    rules = ENTRY.findall(block)
    if not rules:
        raise SystemExit("gen-risk-rules: no rules parsed from risk.rs")
    return rules


def contextual_rules() -> list[str]:
    text = SOURCE.read_text(encoding="utf-8")
    return sorted(set(re.findall(r'rule:\s*"([a-z_]+)"\.to_owned\(\)', text)))


def render(rules: list[tuple[str, str, str]], contextual: list[str]) -> str:
    out: list[str] = []
    out.append("# Risk rules")
    out.append("")
    out.append(
        "Generated from `crates/core/src/risk.rs` by `docs-site/scripts/gen-risk-rules.py`. "
        "Do not edit by hand: CI regenerates this page and fails on any diff."
    )
    out.append("")
    out.append(
        "Classification is an exact, case-sensitive match on the **verified** function name. "
        "A function name that matches no rule is `UNVERIFIED` — never downgraded to `LOW`. "
        "Every classification exposes the rule that matched, so no tier is an opaque score."
    )
    out.append("")
    out.append(f"The table holds **{len(rules)} rules** across {len(TIER_ORDER)} tiers.")
    out.append("")
    for tier in TIER_ORDER:
        tier_rules = [r for r in rules if r[1] == tier]
        if not tier_rules:
            continue
        out.append(f"## {tier} — {len(tier_rules)} rule{'s' if len(tier_rules) != 1 else ''}")
        out.append("")
        out.append(f"_{TIER_NOTE[tier]}_")
        out.append("")
        out.append("| Function name | Why it is this tier |")
        out.append("|---|---|")
        for name, _tier, why in tier_rules:
            out.append(f"| `{name}` | {why} |")
        out.append("")
    out.append("## Contextual rules")
    out.append("")
    out.append(
        "The rules above match on the function name. These additional rules evaluate the decoded "
        "arguments and emit every rule that fired, each with its evidence:"
    )
    out.append("")
    for name in contextual:
        out.append(f"- `{name}`")
    out.append("")
    out.append(
        "Contextual rules never change the function-name tier; they add evidence next to it. "
        "See `docs-site/src/parser/decoding-rules.md` for what a caller receives."
    )
    out.append("")
    return "\n".join(out)


def main() -> int:
    content = render(read_rules(), contextual_rules())
    check = "--check" in sys.argv[1:]
    if check:
        current = TARGET.read_text(encoding="utf-8") if TARGET.exists() else ""
        if current != content:
            print(f"FAIL {TARGET.relative_to(ROOT)} is stale; run gen-risk-rules.py", file=sys.stderr)
            return 1
        print("ok risk-rules page is current")
        return 0
    TARGET.write_text(content, encoding="utf-8")
    print(f"wrote {TARGET.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
