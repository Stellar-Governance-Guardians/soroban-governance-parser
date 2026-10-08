# Draft Wave issue — rent-estimate research for dry-run

**Suggested labels:** `area:research`, `area:simulate`, `complexity:high` · **Complexity:** High (200 pts)

## Summary

`docs/dry-run.md` records that rent is **not modeled**: the committed
`simulateTransaction` captures contain no rent figure, and deriving one from the
response is not obvious. Either establish a correct, documented derivation from
observed data, or prove and record precisely why it cannot be derived from the
simulation response alone. This is a research task with a concrete deliverable —
the answer may legitimately be "not derivable from these fields".

## Acceptance Criteria

- [ ] A written finding in `docs/dry-run.md` (or a new `docs/rent.md`) that
      either:
      (a) gives a derivation of rent from observed fields (e.g. entry durability
      and TTL in `stateChanges` plus the protocol rent parameters), with the
      source for every constant, and an implementation behind a new
      `rent_stroops: Option<String>` field; **or**
      (b) proves the response lacks the inputs (entry size, TTL, protocol
      config), cites the protocol/source, and keeps the field `null`.
- [ ] If implemented: offline tests over committed captures assert the value and
      `null` where it cannot be computed; the schema gains the additive field.
- [ ] If not implemented: the README/`docs/dry-run.md` limitation is updated with
      the exact reason and what a future capture would need to include.
- [ ] No number is ever fabricated: unknown stays `null`.

## Tech Stack

Rust 1.96, `stellar-xdr`, `serde_json`. Protocol references: the pinned
`stellar-core`/Soroban rent parameters (cite the source; do not guess).

## Notes / risks

This is the highest-uncertainty issue in the set. A well-argued negative result
is a valid, complete outcome and should be accepted as such.
