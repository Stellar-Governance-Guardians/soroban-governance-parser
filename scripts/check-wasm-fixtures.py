#!/usr/bin/env python3
"""Offline claim check: every committed seed-v2 upstream wasm fixture must hash
exactly as recorded in scripts/upstream/upstream.lock.json. No network, no
build: it verifies the committed artifacts against their pinned sha256, which is
what build-upstream.sh reproduces from a clean clone. Exit 0 = all match."""
import hashlib
import json
import pathlib
import sys

root = pathlib.Path(__file__).resolve().parent.parent
lock_path = root / "scripts" / "upstream" / "upstream.lock.json"
wasm_dir = root / "tests" / "fixtures" / "seed-v2" / "wasms"

lock = json.loads(lock_path.read_text())
bad = 0
checked = 0
for governor in lock["governors"]:
    for artifact in governor["artifacts"]:
        path = wasm_dir / artifact["file"]
        checked += 1
        if not path.exists():
            print(f"MISSING {artifact['file']}")
            bad += 1
            continue
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != artifact["sha256"]:
            print(f"MISMATCH {artifact['file']}: {actual} != {artifact['sha256']}")
            bad += 1
        else:
            print(f"ok {artifact['file']}")

print(f"wasm fixtures: {checked - bad}/{checked} match upstream.lock.json")
sys.exit(1 if bad else 0)
