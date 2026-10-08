#!/usr/bin/env python3
"""CI gate: changes to schemas/governance-v1.graphql must be additive.

Compares the committed baseline snapshot (schemas/governance-v1.baseline.graphql)
against the current SDL (schemas/governance-v1.graphql). Within v1 the contract
is additive-only:

  * every type / input / enum / scalar in the baseline still exists;
  * every field or argument keeps its name and exact type signature;
  * every enum value still exists.

Adding types, fields, arguments and enum values is allowed. Removing or
retyping anything is a v1 break and fails closed here.

Usage: scripts/check-schema-additive.py [--baseline FILE] [--current FILE]
"""
import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_BASELINE = ROOT / "schemas" / "governance-v1.baseline.graphql"
DEFAULT_CURRENT = ROOT / "schemas" / "governance-v1.graphql"

MEMBER_RE = re.compile(r"([A-Za-z_]\w*)\s*(\(.*?\))?\s*:\s*([A-Za-z_][\w\[\]!.]*)", re.S)


def strip(text: str) -> str:
    # Block descriptions, then line comments.
    text = re.sub(r'"""(?:.|\n)*?"""', "", text)
    text = re.sub(r"#[^\n]*", "", text)
    return text


def parse(text: str):
    """Return (types, enums). types[name] = {member: signature}, enums[name] = set."""
    text = strip(text)
    types: dict[str, dict[str, str]] = {}
    enums: dict[str, set[str]] = {}

    kinds = re.compile(r"\b(type|enum|input|scalar|interface|union|schema)\s+([A-Za-z_]\w*)")
    for m in kinds.finditer(text):
        kind, name = m.group(1), m.group(2)
        rest = text[m.end():]
        j = 0
        while j < len(rest) and rest[j].isspace():
            j += 1
        if j >= len(rest) or rest[j] != "{":
            # scalar (or a directive-only definition): no members
            types.setdefault(name, {})
            continue
        depth, k = 0, j
        while k < len(rest):
            if rest[k] == "{":
                depth += 1
            elif rest[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        body = rest[j + 1 : k]

        if kind == "enum":
            values = {v for v in body.split() if v}
            enums[name] = values
            types.pop(name, None)
        else:
            members: dict[str, str] = {}
            for fm in MEMBER_RE.finditer(body):
                field, args, ty = fm.group(1), fm.group(2), fm.group(3)
                norm_args = re.sub(r"\s+", " ", args).strip() if args else ""
                members[field] = f"{norm_args}:{ty}"
            types[name] = members

    return types, enums


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--baseline", default=str(DEFAULT_BASELINE))
    ap.add_argument("--current", default=str(DEFAULT_CURRENT))
    args = ap.parse_args()

    baseline_path = pathlib.Path(args.baseline)
    current_path = pathlib.Path(args.current)
    if not baseline_path.is_file():
        print(f"FAIL: missing baseline {baseline_path}")
        sys.exit(1)
    if not current_path.is_file():
        print(f"FAIL: missing current SDL {current_path}")
        sys.exit(1)

    base_types, base_enums = parse(baseline_path.read_text())
    cur_types, cur_enums = parse(current_path.read_text())

    problems: list[str] = []

    for name, members in base_types.items():
        if name not in cur_types:
            problems.append(f"type `{name}` was removed or changed kind")
            continue
        for field, sig in members.items():
            if field not in cur_types[name]:
                problems.append(f"`{name}.{field}` was removed")
            elif cur_types[name][field] != sig:
                problems.append(
                    f"`{name}.{field}` changed signature "
                    f"{sig!r} -> {cur_types[name][field]!r}"
                )

    for name, values in base_enums.items():
        if name not in cur_enums:
            problems.append(f"enum `{name}` was removed or changed kind")
            continue
        for value in sorted(values - cur_enums[name]):
            problems.append(f"enum value `{name}.{value}` was removed")

    if problems:
        print("FAIL: non-additive v1 schema change:")
        for p in problems:
            print(f" - {p}")
        sys.exit(1)

    added_types = set(cur_types) - set(base_types)
    added_enums = set(cur_enums) - set(base_enums)
    added_fields = sum(
        len(set(cur_types[t]) - set(base_types.get(t, {}))) for t in cur_types
    )
    print(
        "schema additive check: OK "
        f"(+{len(added_types)} types, +{len(added_enums)} enums, +{added_fields} fields since baseline)"
    )


if __name__ == "__main__":
    main()
