# Draft Wave issue — risk rules for token approvals / caller identity

**Suggested labels:** `area:risk`, `complexity:medium` · **Complexity:** Medium (150 pts)

## Summary

The contextual risk rule set (`crates/core/src/risk.rs`) publishes four rules:
`treasury_outflow`, `self_call`, `large_value`, `batched_actions`. Two obvious
gaps are not covered: an unbounded/allowance-granting call (`approve`) and a call
whose **caller** is a treasury but whose target is external. Extend the published
rule table, keeping the charter rule: rules are data, every flag carries evidence,
and nothing is a composite score.

## Acceptance Criteria

- [ ] Add at least two new rules, e.g.:
  - `allowance_granted` — `approve`/`increase_allowance` with an amount at or
    above a configured threshold (evidence: spender, amount, threshold).
  - `caller_is_treasury` — a value-moving call whose *caller* (not `args[0]`) is a
    registered treasury (needs a `caller` field on `CallUnderReview`).
- [ ] One positive **and** one negative test per new rule, mirroring the existing
  contextual tests.
- [ ] `RiskContext` gains any new fields with `#[serde(default)]` so the wasm
  surface's documented `{}` default keeps working.
- [ ] Rules documented in README/SPEC and machine-checked (extend the
  `risk-contextual-rules-published` claim).
- [ ] No opaque composite identifier introduced (the `risk-rules-published`
  claim still passes).

## Tech Stack

Rust 1.96, `serde`/`serde_json`. Pure functions, no IO; tests are ordinary unit
tests. New fields are additive to the (frozen) `RiskContext` serialization.

## Notes / risks

Adding a `caller` field changes the serialized `RiskContext`; because it is not
part of `schemas/`, no lock bump is needed — but call it out in the PR.
