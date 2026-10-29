#!/usr/bin/env python3
"""Check that every external link in the built site resolves.

NIGHTLY TIER ONLY — this needs the network, so it is not part of the PR gate.
Scans the built HTML under docs-site/book, not the markdown sources, so the URLs
checked are exactly the ones a reader can click.

Local example URLs (localhost / 127.0.0.1) are skipped: they are documentation
samples, not links. Everything else must answer 2xx after following redirects.

Usage: check-external-links.py [--timeout 20] [--concurrency 8]
"""

from __future__ import annotations

import argparse
import re
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

SITE = Path(__file__).resolve().parents[1]
BOOK = SITE / "book"
URL = re.compile(r'(?:href|src)="(https?://[^"#]+)')
SKIP_HOSTS = ("localhost", "127.0.0.1", "0.0.0.0")


def collect() -> list[str]:
    if not BOOK.exists():
        raise SystemExit("check-external-links: docs-site/book is missing; run mdbook build")
    found: set[str] = set()
    for page in BOOK.rglob("*.html"):
        found.update(URL.findall(page.read_text(encoding="utf-8", errors="ignore")))
    return sorted(
        url for url in found if not any(host in url for host in SKIP_HOSTS)
    )


def probe(url: str, timeout: float) -> tuple[bool, str]:
    headers = {"User-Agent": "sgg-docs-link-check/1.0"}
    last = ""
    for attempt in range(3):
        for method in ("HEAD", "GET"):
            request = urllib.request.Request(url, method=method, headers=headers)
            try:
                with urllib.request.urlopen(request, timeout=timeout) as response:
                    status = response.status
                    if 200 <= status < 400:
                        return True, f"{status}"
                    last = f"HTTP {status}"
            except urllib.error.HTTPError as error:
                last = f"HTTP {error.code}"
                # A server that refuses HEAD may still serve GET.
                if method == "GET":
                    break
            except (urllib.error.URLError, TimeoutError, OSError) as error:
                last = type(error).__name__ + f": {error}"
        time.sleep(1.5 * (attempt + 1))
    return False, last


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--timeout", type=float, default=20.0)
    parser.add_argument("--concurrency", type=int, default=8)
    args = parser.parse_args()

    urls = collect()
    if not urls:
        print("check-external-links: no external links found", file=sys.stderr)
        return 1

    with ThreadPoolExecutor(max_workers=args.concurrency) as pool:
        results = list(pool.map(lambda u: (u, *probe(u, args.timeout)), urls))

    failures = [(url, detail) for url, ok, detail in results if not ok]
    for url, detail in failures:
        print(f"FAIL {url} — {detail}", file=sys.stderr)
    print(f"external links: {len(urls) - len(failures)}/{len(urls)} reachable")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
