#!/usr/bin/env python3
"""Verify docs-site/sources.lock.

Default (offline): hash the vendored copies under docs-site/pinned/ and compare
with the lock. This is what the PR gate can do.

--fetch (nightly tier): download each pinned file at its exact commit from
raw.githubusercontent.com and compare bytes with the lock, failing on drift or
an unreachable pin. This is the check that keeps a stale summary from sitting on
the site after a sibling repository changes.

Usage: verify-pins.py [--fetch]
"""

from __future__ import annotations

import hashlib
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path

SITE = Path(__file__).resolve().parents[1]
LOCK = SITE / "sources.lock"
RAW = "https://raw.githubusercontent.com"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def owner_repo(repo: str) -> str:
    parts = repo.rstrip("/").split("/")
    return f"{parts[-2]}/{parts[-1]}"


def main() -> int:
    fetch = "--fetch" in sys.argv[1:]
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    failures = 0

    for entry in lock.get("sources", []):
        name = entry["repo"].rstrip("/").split("/")[-1]
        expected = entry["sha256"]
        if fetch:
            url = f"{RAW}/{owner_repo(entry['repo'])}/{entry['commitSha']}/{entry['path']}"
            try:
                with urllib.request.urlopen(
                    urllib.request.Request(
                        url, headers={"User-Agent": "sgg-docs-pin-check/1.0"}
                    ),
                    timeout=30,
                ) as response:
                    data = response.read()
            except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as error:
                print(f"FAIL {name}: pinned URL unreachable — {url} — {error}", file=sys.stderr)
                failures += 1
                continue
            actual = sha256(data)
            if actual != expected:
                print(
                    f"FAIL {name}: drift at {entry['commitSha'][:12]} — "
                    f"{entry['path']} sha256 {actual} != pinned {expected}",
                    file=sys.stderr,
                )
                failures += 1
            else:
                print(f"ok {name} @ {entry['commitSha'][:12]} ({actual[:12]}…)")
        else:
            vendored = SITE / "pinned" / name / entry["path"]
            if not vendored.exists():
                print(f"FAIL {name}: vendored copy missing at {vendored}", file=sys.stderr)
                failures += 1
                continue
            actual = sha256(vendored.read_bytes())
            if actual != expected:
                print(
                    f"FAIL {name}: vendored {entry['path']} sha256 {actual} != pinned {expected}",
                    file=sys.stderr,
                )
                failures += 1
            else:
                print(f"ok {name} {entry['path']} ({actual[:12]}…)")

    mode = "fetched" if fetch else "offline"
    print(f"pins ({mode}): {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
