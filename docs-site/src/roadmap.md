# Roadmap

Ordered by dependency, not by desire: each step unblocks the next.

## Done

| Step | State |
|---|---|
| ScVal → JSON, `contractspecv0`, risk rules, `GovernorAdapter`, power/tally replicas | implemented, differentially tested |
| Script3 adapter | implemented, tested against committed on-chain reads |
| v1 schema frozen, additive-only gate | implemented |
| WASM package `v0.1.0-alpha.1` with a reproducible sha256 | released as a pre-release |
| Dry-run model from committed real captures | implemented, offline-tested |
| Indexer ingestion (I0–I2) | implemented, integration-tested |
| Dashboard scaffold: `/`, `/simulate`, `/status` | merged, CI green |
| This documentation site | built and deployed from the parser repo |

## Next

| Step | Depends on | Status |
|---|---|---|
| Dependency refresh (TLS/async advisories) | — | open |
| Parser WASM `v0.1.0-alpha.2`: adapter event surface + simulation request/response | dependency refresh | planned |
| Indexer I3: decode through the pinned adapter | alpha.2 asset | planned |
| Indexer I4: GraphQL implementing the vendored SDL, with metrics per [Metrics](metrics.md) | I3 | planned |
| Indexer I5–I7: reconciliation, deploy recipe, clean-clone proof | I4 | planned |
| Dashboard data pages, codegen, live-indexer e2e | I4 | planned |
| OpenZeppelin adapter | fixtures already committed | planned, bounded effort |
| 72-hour soak on a host | deploy recipe + a host | blocked on infrastructure |

## Not scheduled

- Vote casting or any write path: the suite is read-only.
- A composite trust score for delegates: metrics stay raw counts plus formulas.
- Mainnet deployments: fixtures and evidence are testnet.

Dates are deliberately absent. Each row moves when its dependency lands and its
evidence exists, and this page is updated in the same PR.
