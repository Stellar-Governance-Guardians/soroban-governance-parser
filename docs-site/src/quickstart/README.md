# Quickstart

Each repository runs on its own and has its own gate. Nothing here needs a
sibling checkout: cross-repo inputs are pinned artifacts.

| Repository | Toolchain | Time to a green check | Start here |
|---|---|---|---|
| parser | Rust 1.96.0, Node 24 for WASM tests | a few minutes for a cold build | [Parser](parser.md) |
| indexer | Node ≥ 24, Postgres 16 | a few minutes | [Indexer](indexer.md) |
| dashboard | Node ≥ 24, Chrome for e2e | a few minutes | [Dashboard](dashboard.md) |

The PR gate in every repository is offline and deterministic. Live-testnet
checks run in a nightly or manual workflow and never block a merge.
