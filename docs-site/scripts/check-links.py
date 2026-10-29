#!/usr/bin/env python3
"""Offline internal-link check for docs-site/src.

Fails on a markdown link whose target file does not exist in the book source, or
whose #anchor does not match a heading in the target. External links (http,
https, mailto) are not fetched here: the nightly docs-live workflow owns those.

Usage: check-links.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

SITE = Path(__file__).resolve().parents[1]
SRC = SITE / "src"

LINK = re.compile(r"\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
HEADING = re.compile(r"^(#{1,6})\s+(.*)$", re.MULTILINE)


def slug(text: str) -> str:
    """Approximate mdBook's heading id: lowercase, drop punctuation, spaces to -."""
    text = re.sub(r"`([^`]*)`", r"\1", text)
    text = re.sub(r"\*\*|__|\*|_", "", text)
    text = text.strip().lower()
    text = re.sub(r"[^\w\s-]", "", text, flags=re.UNICODE)
    text = re.sub(r"\s+", "-", text)
    return text.strip("-")


def anchors(path: Path) -> set[str]:
    text = path.read_text(encoding="utf-8")
    found = {slug(match.group(2)) for match in HEADING.finditer(text)}
    # mdBook also gives every page an implicit id for duplicate headings; the
    # set above is enough to catch a link to a heading that does not exist.
    return found


def main() -> int:
    failures: list[str] = []
    pages = sorted(SRC.rglob("*.md"))
    if not pages:
        print("check-links: no markdown sources found", file=sys.stderr)
        return 1

    for page in pages:
        text = page.read_text(encoding="utf-8")
        for match in LINK.finditer(text):
            target = match.group(1).strip()
            if target.startswith(("http://", "https://", "mailto:", "#")):
                if target.startswith("#"):
                    anchor = target[1:]
                    if anchor and anchor not in anchors(page):
                        failures.append(
                            f"{page.relative_to(SITE)}: missing anchor #{anchor} in this page"
                        )
                continue

            path_part, _, anchor = target.partition("#")
            if path_part == "":
                continue
            if path_part.startswith("/"):
                failures.append(
                    f"{page.relative_to(SITE)}: absolute link {target} is not portable on Pages"
                )
                continue
            resolved = (page.parent / path_part).resolve()
            if not resolved.exists():
                failures.append(f"{page.relative_to(SITE)}: {target} does not exist")
                continue
            if anchor and resolved.suffix == ".md":
                if anchor not in anchors(resolved):
                    failures.append(
                        f"{page.relative_to(SITE)}: {target} — no heading #{anchor} in "
                        f"{resolved.relative_to(SITE)}"
                    )

    # Every SUMMARY entry must exist, or mdBook fails late with a worse message.
    summary = (SRC / "SUMMARY.md").read_text(encoding="utf-8")
    for match in re.finditer(r"\]\(([^)]+\.md)\)", summary):
        entry = (SRC / match.group(1)).resolve()
        if not entry.exists():
            failures.append(f"SUMMARY.md: missing page {match.group(1)}")

    if failures:
        for failure in failures:
            print(f"FAIL {failure}", file=sys.stderr)
        return 1
    print(f"links: OK ({len(pages)} pages)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
