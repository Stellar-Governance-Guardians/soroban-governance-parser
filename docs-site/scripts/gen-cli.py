#!/usr/bin/env python3
"""Generate docs-site/src/generated/cli.md from `sgp --help` output.

The CLI reference is generated, not written: it is the binary's own help text.
CI rebuilds the binary, regenerates the page and fails on any diff.

Usage: gen-cli.py [--check]
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / "docs-site/src/generated/cli.md"
PACKAGE = "soroban-governance-cli"
BIN = "sgp"


def run(args: list[str]) -> str:
    """Run a command from the repo root and return stdout, failing loudly."""
    proc = subprocess.run(
        args, cwd=ROOT, text=True, capture_output=True, check=False
    )
    if proc.returncode != 0:
        raise SystemExit(
            f"gen-cli: {' '.join(args)} exited {proc.returncode}\n{proc.stderr}"
        )
    return proc.stdout.rstrip() + "\n"


def build() -> Path:
    subprocess.run(
        ["cargo", "build", "-q", "-p", PACKAGE, "--bin", BIN],
        cwd=ROOT,
        check=True,
    )
    binary = ROOT / "target/debug" / BIN
    if not binary.exists():
        raise SystemExit(f"gen-cli: built binary not found at {binary}")
    return binary


def subcommands(help_text: str) -> list[tuple[str, str]]:
    """Parse the `Commands:` section of the top-level help."""
    found: list[tuple[str, str]] = []
    in_commands = False
    for line in help_text.splitlines():
        if line.strip() == "Commands:":
            in_commands = True
            continue
        if in_commands:
            if not line.startswith("  "):
                break
            stripped = line.strip()
            if not stripped:
                continue
            name, _, summary = stripped.partition("  ")
            name = name.strip()
            if name and name != "help":
                found.append((name, summary.strip()))
    return found


def render(binary: Path) -> str:
    version = run([str(binary), "--version"]).strip()
    top = run([str(binary), "--help"])
    subs = subcommands(top)

    out: list[str] = []
    out.append("# CLI reference")
    out.append("")
    out.append(
        "Generated from the `sgp` binary (`crates/cli`) by "
        "`docs-site/scripts/gen-cli.py`. Do not edit by hand: CI regenerates this "
        f"page and fails on any diff. Binary version: `{version}`."
    )
    out.append("")
    out.append(
        "`sgp` is the operator tool. Network IO lives in `crates/cli/src/rpc.rs`; "
        "the core crate stays sans-IO."
    )
    out.append("")
    out.append("## Top-level")
    out.append("")
    out.append("```text")
    out.append(top.rstrip())
    out.append("```")
    out.append("")
    for name, summary in subs:
        out.append(f"## `sgp {name}`")
        out.append("")
        if summary:
            out.append(summary)
            out.append("")
        out.append("```text")
        out.append(run([str(binary), name, "--help"]).rstrip())
        out.append("```")
        out.append("")
    return "\n".join(out)


def main() -> int:
    binary = build()
    content = render(binary)
    check = "--check" in sys.argv[1:]
    if check:
        current = TARGET.read_text(encoding="utf-8") if TARGET.exists() else ""
        if current != content:
            print(f"FAIL {TARGET.relative_to(ROOT)} is stale; run gen-cli.py", file=sys.stderr)
            return 1
        print("ok cli page is current")
        return 0
    TARGET.write_text(content, encoding="utf-8")
    print(f"wrote {TARGET.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
