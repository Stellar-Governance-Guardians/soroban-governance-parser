# Indexer

`governance-event-indexer` moves governance events from the public Stellar
testnet RPC into Postgres — raw events are preserved and parser-backed event
decodes are stored durably — and serves an initial read-only GraphQL API for
`governance-v1` schema.

> **Status: ingestion, durable event decode, the initial GraphQL surface, the
> deploy recipe (I6) and the clean-clone prove script (I7) are built. Proposal,
> vote and delegate projections and reconciliation (I5) remain planned.** The
> GraphQL endpoint serves health and normalized event records; proposal, impact
> and delegate fields remain unavailable until their tables are implemented.

## What is built

### Registry and pins

- The pinned `deployments.json` from the parser repo, hash-verified on every
  load; `GOVERNOR_REGISTRY_OVERRIDE` adds contract ids at runtime from validated
  strkeys, failing closed on anything malformed.
- Three pins verified offline in every PR: `parser.lock` (WASM asset),
  `schema.lock` (SDL) and `registry.lock` (deployments). A nightly workflow
  checks that the pinned URLs still resolve to identical bytes.

### Ingestion

- **Windows.** `getEvents` in ranges of at most 1000 ledgers, because the public
  RPC times out on wider ranges; `INGEST_WINDOW_LEDGERS` is rejected above 1000.
- **Cursors.** One persisted cursor per registered contract, initialized at the
  start of the RPC's retention window.
- **Storage.** Raw events land in `raw_events`, keyed
  `(ledger, tx_hash, op_index, event_index)`, inserted idempotently. A window
  commits together with its cursor advance, so a killed process re-fetches and
  converges.
- **Retention gaps.** A cursor behind the RPC's oldest ledger writes the
  unreadable range to `ingest_gaps` with reason `retention` and advances past it
  in the same transaction — loud, never a silent skip.
- **Replay.** `node dist/cli.js replay <dir>` runs committed raw responses
  through the same raw-plus-decode path, tagging rows `source='fixture-replay'`.
  Live rows are `source='live'`, and a later write never flips a row's source.

### Storage

Five tables exist after `migrate` (forward-only, tested from an empty database):

| Table | Holds |
|---|---|
| `raw_events` | every ingested event, keyed `(ledger, tx_hash, op_index, event_index)`, with `source` = `live` or `fixture-replay` |
| `decoded_events` | parser-versioned normalized topics/payload and explicit `VERIFIED` or `UNVERIFIED` status for each raw event |
| `ingest_cursors` | one cursor per registered contract, advanced in the same transaction as its window |
| `ingest_gaps` | unreadable ledger ranges with the reason (`retention`), written instead of skipping silently |
| `indexer_meta` | indexer bookkeeping (schema version and run metadata) |

Proposal-level tables (`proposals`, `proposal_actions`, `risk_flags`, `votes`,
`power_checkpoints`, `delegations`, `delegates`, `reconciliation_runs`) arrive
with I5 and later. Event-level decode is persisted in `decoded_events`.

### Retry behaviour

Bounded exponential backoff with jitter on RPC failures. There is no unbounded
retry loop: a failure that exceeds the bound surfaces as an error, and the
cursor does not advance past data that was not stored.

## What is not built

| Milestone | State |
|---|---|
| I3 decode through the pinned Script3 adapter | event-level decode persisted through the pinned WASM; proposal projections remain planned |
| I4 GraphQL (Yoga) implementing the vendored SDL exactly | initial health and event connection implemented; proposal/impact/delegate fields remain planned |
| I5 reconciliation of SQL tallies against on-chain reads | not built; drift checks remain planned |
| I6 Docker/compose deploy recipe | recipe and `deploy.yml` shipped; no instance has been deployed |
| I7 clean-clone [prove script](https://github.com/Stellar-Governance-Guardians/governance-event-indexer/blob/d9a755c8cb4e02c4c4bf55ea1c9131a1e6ea6ca5/scripts/prove-indexer) | built; asserts GraphQL values against the committed fixture replay, never yet run against a live indexer |

Decoding governor events in TypeScript would put governor logic outside the
parser's `GovernorAdapter` and break the project's rule that governor logic
lives in the parser. The event-level boundary now uses the pinned parser
artifact; remaining work is the durable projection of proposal entities.

## Checks

| Command | What it proves |
|---|---|
| `npm run check-pins` | vendored bytes match all three locks, offline |
| `npm run typecheck`, `npm run lint` | strict TypeScript and ESLint pass |
| `npm run test:unit` | 59 tests, no database, no network |
| `npm run test:integration` | 18 tests: migrations from empty, windowing, idempotency, restart safety, gaps, replay |

## Provenance

The indexer consumes the parser through pins: the SDL and registry at parser
commit `ba3bf1a3…`, the WASM asset `v0.1.0-alpha.2`. Its README pin table is the
authoritative list; this page summarises it, and the revisions this summary was
written against are recorded in `docs-site/sources.lock`.
