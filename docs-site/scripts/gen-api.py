#!/usr/bin/env python3
"""Generate docs-site/src/generated/api.md from schemas/governance-v1.graphql.

The SDL is the single source of truth shared by the indexer (server) and the
dashboard (client). This page is generated from it, never hand-written: CI
regenerates it and fails on any diff.

Usage: gen-api.py [--check]
"""

from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "schemas/governance-v1.graphql"
TARGET = ROOT / "docs-site/src/generated/api.md"

HEADER = re.compile(r"^(schema|type|enum|scalar|input|interface|union)\b\s*([A-Za-z_][A-Za-z0-9_]*)?\s*(.*)$")
FIELD = re.compile(
    r"^([A-Za-z_][A-Za-z0-9_]*)\s*(?:\((.*)\))?\s*:\s*(.+?)(?:\s*=\s*(.+))?$"
)
ARG = re.compile(
    r"([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(\[[^\]]*\]|[A-Za-z_][A-Za-z0-9_]*!?)\s*(?:=\s*([^,)\s]+))?"
)


class Definition:
    def __init__(self, kind: str, name: str, description: str | None) -> None:
        self.kind = kind
        self.name = name
        self.description = description
        self.entries: list[tuple[str | None, str]] = []


def strip_line_comment(line: str) -> str:
    """Remove a trailing # comment that is not inside a description."""
    marker = line.find("#")
    return line if marker < 0 else line[:marker]


def parse(text: str) -> list[Definition]:
    lines = text.splitlines()
    definitions: list[Definition] = []
    pending: str | None = None
    in_description = False
    buffer: list[str] = []
    i = 0

    def flush_description() -> None:
        nonlocal pending
        joined = " ".join(part.strip() for part in buffer).strip()
        pending = joined if joined else None
        buffer.clear()

    while i < len(lines):
        raw = lines[i]
        stripped = raw.strip()
        i += 1

        if in_description:
            if '"""' in stripped:
                head, _, _tail = stripped.partition('"""')
                if head.strip():
                    buffer.append(head)
                in_description = False
                flush_description()
            else:
                buffer.append(stripped)
            continue

        if stripped.startswith('"""'):
            rest = stripped[3:]
            if rest.endswith('"""'):
                pending = rest[: -3].strip() or None
            else:
                buffer.clear()
                if rest.strip():
                    buffer.append(rest)
                in_description = True
            continue

        stripped = strip_line_comment(stripped).strip()
        if not stripped:
            continue

        match = HEADER.match(stripped)
        if match and not stripped.startswith(("query", "mutation", "subscription")):
            kind, name, tail = match.group(1), match.group(2) or "", match.group(3).strip()
            definition = Definition(kind, name, pending)
            pending = None

            if kind == "scalar":
                definitions.append(definition)
                continue

            if "{" not in tail:
                # One-line union: union Name = A | B
                if kind == "union" and "=" in tail:
                    definition.entries = [(None, part.strip()) for part in tail.split("=", 1)[1].split("|")]
                definitions.append(definition)
                continue

            # Collect the body until the closing brace at this nesting level.
            body: list[str] = []
            depth = tail.count("{") - tail.count("}")
            after_brace = tail.split("{", 1)[1]
            if after_brace.strip():
                body.append(after_brace)
            while depth > 0 and i < len(lines):
                line = lines[i]
                i += 1
                depth += line.count("{") - line.count("}")
                if depth <= 0:
                    body.append(line.split("}", 1)[0])
                else:
                    body.append(line)
            definition.entries = parse_entries(body)
            definitions.append(definition)
            continue

        # Description before a field/entry that we somehow landed on at top level.
        pending = None

    return definitions


def parse_entries(body: list[str]) -> list[tuple[str | None, str]]:
    entries: list[tuple[str | None, str]] = []
    pending: str | None = None
    in_description = False
    buffer: list[str] = []
    accumulating = ""

    for raw in body:
        stripped = raw.strip()

        if in_description:
            if '"""' in stripped:
                head = stripped.split('"""', 1)[0]
                if head.strip():
                    buffer.append(head)
                pending = " ".join(part.strip() for part in buffer).strip() or None
                buffer.clear()
                in_description = False
            else:
                buffer.append(stripped)
            continue

        if stripped.startswith('"""'):
            rest = stripped[3:]
            if rest.endswith('"""'):
                pending = rest[: -3].strip() or None
            else:
                buffer = [rest] if rest.strip() else []
                in_description = True
            continue

        stripped = strip_line_comment(stripped).strip()
        if not stripped:
            continue

        accumulating = f"{accumulating} {stripped}".strip()
        if accumulating.count("(") > accumulating.count(")"):
            continue
        entries.append((pending, accumulating))
        pending = None
        accumulating = ""

    if accumulating:
        entries.append((pending, accumulating))
    return entries


def format_args(raw: str | None) -> str:
    if not raw:
        return ""
    # SDL arguments are whitespace-separated, not comma-separated.
    rendered: list[str] = []
    for match in ARG.finditer(raw):
        name, type_ref, default = match.group(1), match.group(2).strip(), match.group(3)
        default_text = f" = {default.strip()}" if default else ""
        rendered.append(f"`{name}`: `{type_ref}`{default_text}")
    return "<br>".join(rendered)


def render(definitions: list[Definition], sdl_sha: str) -> str:
    out: list[str] = []
    out.append("# API reference (`governance-v1`)")
    out.append("")
    out.append(
        "Generated from `schemas/governance-v1.graphql` by "
        f"`docs-site/scripts/gen-api.py`. Do not edit by hand; CI regenerates it "
        f"and fails on any diff. SDL sha256: `{sdl_sha}`."
    )
    out.append("")
    out.append(
        "This SDL is the shared interface contract. The indexer serves it; the "
        "dashboard validates every response field against it. Bumping it requires "
        "a `schema.lock` update in each consumer repo, and the parser's PR gate "
        "refuses non-additive changes to the v1 schema."
    )
    out.append("")
    out.append(
        "Server status: **not served yet.** The indexer currently stops at raw "
        "event ingestion (I2); the GraphQL server (I4) is planned. Response "
        "examples on this page are the schema, not captured responses."
    )
    out.append("")

    for definition in definitions:
        heading = f"## `{definition.kind}` {definition.name}".rstrip()
        out.append(heading)
        out.append("")
        if definition.description:
            out.append(definition.description)
            out.append("")

        if definition.kind == "scalar":
            out.append("Opaque scalar serialized as JSON.")
            out.append("")
            continue

        if definition.kind == "union":
            members = ", ".join(f"`{name}`" for _desc, name in definition.entries)
            out.append(f"Members: {members}.")
            out.append("")
            continue

        if definition.kind == "schema":
            roots = ", ".join(f"`{entry}`" for _desc, entry in definition.entries)
            out.append(f"Root operations: {roots}.")
            out.append("")
            continue

        if definition.kind == "enum":
            out.append("| Value | Description |")
            out.append("|---|---|")
            for desc, value in definition.entries:
                out.append(f"| `{value}` | {desc or ''} |")
            out.append("")
            continue

        # type / interface / input
        out.append("| Field | Type | Arguments | Description |")
        out.append("|---|---|---|---|")
        for desc, entry in definition.entries:
            match = FIELD.match(entry)
            if not match:
                continue
            name, args, type_ref, default = match.groups()
            default_text = f" (default `{default.strip()}`)" if default else ""
            out.append(
                f"| `{name}` | `{type_ref.strip()}`{default_text} | {format_args(args)} | {desc or ''} |"
            )
        out.append("")

    out.append("## Full SDL")
    out.append("")
    out.append("```graphql")
    out.append(SOURCE.read_text(encoding="utf-8").rstrip())
    out.append("```")
    out.append("")
    return "\n".join(out)


def main() -> int:
    raw = SOURCE.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    content = render(parse(raw.decode("utf-8")), digest)
    check = "--check" in sys.argv[1:]
    if check:
        current = TARGET.read_text(encoding="utf-8") if TARGET.exists() else ""
        if current != content:
            print(f"FAIL {TARGET.relative_to(ROOT)} is stale; run gen-api.py", file=sys.stderr)
            return 1
        print("ok api page is current")
        return 0
    TARGET.write_text(content, encoding="utf-8")
    print(f"wrote {TARGET.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
