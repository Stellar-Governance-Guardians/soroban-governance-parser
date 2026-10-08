# Draft Wave issue — Soroban custom-type pretty-printers

**Suggested labels:** `area:decode`, `complexity:medium` · **Complexity:** Medium (150 pts)

## Summary

`contractspecv0` declares user-defined types (structs, unions, enums) and the
parser already renders their `ScVal` shape faithfully, but the JSON is raw: a
union shows up as a positional `Vec`, a struct as an anonymous map. Add
renderers that turn values whose type is known from a contract spec into
**named, readable** JSON — while keeping the current raw output for anything the
spec does not cover.

## Acceptance Criteria

- [ ] Given a spec and a function's args, structs render as a JSON object keyed
      by field name; unions/enums render as `{ "variant": <name>, "value": ... }`.
- [ ] Rendering is only applied when the type is resolvable from the spec; any
      closure/cycle or missing type falls back to the existing raw form and is
      marked `unverified` — never guessed.
- [ ] Unit tests cover a nested struct, an enum, a union with a void arm, and an
      unresolvable reference.
- [ ] A fixture-driven test renders a real captured argument (e.g. the Script3
      `Calldata` from the committed `propose` capture) and matches the expected
      named shape.
- [ ] README "What it does" notes the renderer.

## Tech Stack

Rust 1.96, `stellar-xdr` (`ScSpecTypeDef`/`ScSpecTypeDef` resolution),
`serde_json`. Pure, no IO.

## Notes / risks

Keep the raw `ScVal → JSON` converter untouched and total; the pretty-printer is
a layer on top so nothing regresses for unspec'd contracts.
