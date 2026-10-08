#!/usr/bin/env python3
"""Offline claim: README's stated WASM sha256 matches the committed lock file.

Also checks the asset name agrees, and — when the built `.tgz` is present in the
working tree — that it hashes to the recorded sha256. The asset itself is a
GitHub Release asset, not committed to git, so its absence is not a failure.

Exit 0 = claim holds; non-zero with a message = it does not (fail closed).
"""
import hashlib
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOCK = ROOT / "releases" / "parser-wasm.lock.json"
README = ROOT / "README.md"


def fail(msg: str) -> None:
    print(f"FAIL: {msg}")
    sys.exit(1)


def main() -> None:
    if not LOCK.is_file():
        fail(f"missing lock file {LOCK.relative_to(ROOT)}")
    lock = json.loads(LOCK.read_text())

    sha = lock["sha256"]
    asset = lock["assetName"]
    if not re.fullmatch(r"[0-9a-f]{64}", sha):
        fail(f"lock sha256 is not 64-hex: {sha!r}")

    readme = README.read_text()
    if sha not in readme:
        fail(f"README does not state the lock sha256 {sha}")
    if asset not in readme:
        fail(f"README does not name the asset {asset}")

    local = ROOT / asset
    if local.is_file():
        digest = hashlib.sha256(local.read_bytes()).hexdigest()
        if digest != sha:
            fail(f"built {asset} hashes to {digest}, lock says {sha}")
        print(f"ok   built {asset} matches the lock sha256")
    else:
        print(f"ok   README sha256 matches lock ({asset} not present locally)")

    print("wasm-release claim: OK")


if __name__ == "__main__":
    main()
