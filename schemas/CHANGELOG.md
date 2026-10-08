# Schemas changelog

The cross-repo contract is `governance-v1.graphql` (GraphQL SDL) plus
`schema-v1.json` (JSON Schema). Within v1, changes are **additive only**:
consumers (`governance-event-indexer`, `delegate-portal-dashboard`) pin
`schema.lock` and may not be broken by a v1 edit.

Enforcement: `scripts/check-schema-additive.py` diffs the current SDL against
the committed baseline `schemas/governance-v1.baseline.graphql`. It fails if any
baseline type, field, argument signature or enum value is removed or retyped.
Adding types/fields/values is allowed. It runs as the `schema-additive` claim in
the offline PR gate.

A pin is bumped only by a PR that shows the diff and re-runs the contract tests
(interface contract).

## v1.0.0 — 2026-10-08 (frozen)

- Initial frozen v1 contract.
- SDL: `schemas/governance-v1.graphql`; baseline snapshot:
  `schemas/governance-v1.baseline.graphql`; JSON Schema: `schemas/schema-v1.json`.
- Types: `DecodingStatus`, `RiskTier`, `RiskClassification`, `DecodedCall`,
  `DecodedProposal`, `NormalizedEvent`, `ExecutionImpact`, `DelegateMetrics`,
  `IndexerHealth`, `EventConnection`, `EventEdge`, `PageInfo`; root `Query`;
  scalar `JSON`.
- `ExecutionImpact.estimate` is `const true` (charter rule 5); `simulatedAtLedger`
  is mandatory.

### How to change it

1. Add the new field / type / enum value — never remove or retype inside v1.
2. Run `python3 scripts/check-schema-additive.py` (also a PR-gate claim).
3. To make a breaking change, add `governance-v2.graphql` / `schema-v2.json`
   instead and coordinate a lock bump in every consumer repo.
