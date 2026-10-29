#!/usr/bin/env python3
"""Check the deployed site: HTTP 200 and the required pages present.

NIGHTLY TIER. Verifies the URL the project publishes, the pages the deployment
check names, and a marker from each so a soft-200 error page cannot pass.

Usage: check-live-site.py [--base https://…/soroban-governance-parser/]
"""

from __future__ import annotations

import argparse
import sys
import urllib.error
import urllib.request

DEFAULT_BASE = "https://stellar-governance-guardians.github.io/soroban-governance-parser/"

REQUIRED = {
    "": "Stellar Governance Guardians",
    "architecture.html": "Architecture",
    "generated/api.html": "API reference",
    "generated/evidence.html": "Evidence",
}


def fetch(url: str) -> tuple[int, str]:
    request = urllib.request.Request(
        url, headers={"User-Agent": "sgg-docs-live-check/1.0"}
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.status, response.read().decode("utf-8", errors="replace")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", default=DEFAULT_BASE)
    args = parser.parse_args()
    base = args.base if args.base.endswith("/") else args.base + "/"

    failures = 0
    for page, marker in REQUIRED.items():
        url = base + page
        try:
            status, body = fetch(url)
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as error:
            print(f"FAIL {url} — {error}", file=sys.stderr)
            failures += 1
            continue
        if status != 200:
            print(f"FAIL {url} — HTTP {status}", file=sys.stderr)
            failures += 1
        elif marker not in body:
            print(f"FAIL {url} — HTTP 200 but no marker '{marker}'", file=sys.stderr)
            failures += 1
        else:
            print(f"ok {url} (200, marker present)")

    print(f"live site: {len(REQUIRED) - failures}/{len(REQUIRED)} pages OK at {base}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
