# Stellar Governance Guardians

Read Soroban governance on Stellar without trusting a UI: decode the raw
`ScVal` payloads, see which published risk rule matched, index the events into
Postgres, and read delegate behaviour as raw counts plus formulas.

The suite is three public repositories with a strict dependency direction:

```text
soroban-governance-parser  →  governance-event-indexer  →  delegate-portal-dashboard
        (Rust + WASM)                (TypeScript)                (Next.js)
```

| Repository | What it does today | Status |
|---|---|---|
| [parser](quickstart/parser.md) | Decodes `ScVal` → JSON; extracts and parses `contractspecv0`; classifies risk from a published rule table; decodes Script3 proposals and events; models dry-run output from committed captures; ships a WASM package. | Script3 adapter implemented. OpenZeppelin adapter planned. CLI `simulate` command planned. |
| [indexer](indexer.md) | Ingests raw governance events from the public testnet RPC into Postgres with cursors, ≤1000-ledger windows, retention-gap recording and fixture replay. | Ingestion built. Decode, GraphQL API, reconciliation and deploy are planned. |
| [dashboard](dashboard.md) | Decodes a pasted base64 `ScVal` in the browser with the pinned parser WASM; shows indexer health and the exact interface pins. | Scaffold merged: `/`, `/simulate`, `/status`. Data pages are planned. |

## Who this is for

- Indexer and dashboard builders who need decoded governance payloads without
  reimplementing governor logic.
- Delegate-tooling authors who want risk classifications that expose the rule
  that fired instead of a composite score.
- Anyone who needs to read a proposal, a vote or a tally from raw chain data and
  check the reading themselves.

## How this site is built

Everything on this site is either a page in
`docs-site/src/` or generated from a committed source file by a script in
`docs-site/scripts/`:

| Page | Generated from |
|---|---|
| [Risk rules](generated/risk-rules.md) | `crates/core/src/risk.rs` |
| [CLI reference](generated/cli.md) | the `sgp` binary's own `--help` output |
| [API reference](generated/api.md) | `schemas/governance-v1.graphql` |
| [Evidence](generated/evidence.md) | `deployments.json` |
| [Upstream pins](generated/upstream.md) | `docs-site/sources.lock` |

CI rebuilds every generated page and fails on any diff, so a page cannot
describe a rule table, an export list, a schema or a contract that the source
files do not contain. Prose claims are checked by
`docs-site/scripts/check-docs-claims`.

## What is not built

The [honest limitations](limitations.md) page aggregates every gap across the
three repositories — including the OpenZeppelin adapter, the missing GraphQL
server and the dashboard's planned data pages. Nothing on this site describes a
planned feature as if it existed; planned pages carry a **Status: planned**
banner.

## License

MIT, in all three repositories.
