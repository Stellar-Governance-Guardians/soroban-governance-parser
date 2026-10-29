#!/usr/bin/env python3
"""Generate docs-site/src/generated/upstream.md from docs-site/sources.lock.

The page is generated so the pins it displays can never drift from the lock
file. The lock is verified by check-docs-claims.py (offline, against the vendored
copies) and by the nightly docs-live workflow (against the pinned raw URLs).

Usage: gen-upstream.py [--check]
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

SITE = Path(__file__).resolve().parents[1]
LOCK = SITE / "sources.lock"
TARGET = SITE / "src/generated/upstream.md"


def render(lock: dict) -> str:
    sources = lock.get("sources", [])
    out: list[str] = []
    out.append("# Upstream pins")
    out.append("")
    out.append(
        "Generated from `docs-site/sources.lock` by "
        "`docs-site/scripts/gen-upstream.py`. Do not edit by hand."
    )
    out.append("")
    out.append(
        "The indexer and dashboard pages on this site are summaries written in "
        "this repository, not embedded copies of those repositories' own "
        "documentation. Each summary names the exact revision it was written "
        "against below, and the bytes of that revision are vendored under "
        "`docs-site/pinned/` and hashed by the PR gate. The nightly `docs-live` "
        "workflow re-fetches the pinned file and fails if the content no longer "
        "matches — so a silent change upstream cannot leave this site claiming "
        "something stale."
    )
    out.append("")
    out.append("| Repository | Path | Commit | sha256 |")
    out.append("|---|---|---|---|")
    for entry in sources:
        out.append(
            f"| {entry['repo']} | `{entry['path']}` | `{entry['commitSha']}` | "
            f"`{entry['sha256']}` |"
        )
    out.append("")
    out.append(
        "Bumping a pin is a parser-repo PR that shows the vendored byte diff and "
        "the review on this site that the summary still matches. This happens at "
        "the end of every phase."
    )
    out.append("")
    out.append("## What is not pinned")
    out.append("")
    out.append(
        "Nothing from a sibling repository is rendered as prose on this site "
        "today. If a later phase embeds sibling documentation directly, it must "
        "enter through this lock file first."
    )
    out.append("")
    return "\n".join(out)


def main() -> int:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    content = render(lock)
    check = "--check" in sys.argv[1:]
    if check:
        current = TARGET.read_text(encoding="utf-8") if TARGET.exists() else ""
        if current != content:
            print(f"FAIL {TARGET} is stale; run gen-upstream.py", file=sys.stderr)
            return 1
        print("ok upstream page is current")
        return 0
    TARGET.write_text(content, encoding="utf-8")
    print(f"wrote {TARGET.relative_to(SITE.parent)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
