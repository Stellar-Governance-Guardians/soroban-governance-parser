# Indexer

`governance-event-indexer` moves governance events from the public Stellar
testnet RPC into Postgres — raw events land before anything is decoded — and is
planned to serve them over a GraphQL API that implements the pinned
`governance-v1` schema.

> **Status: ingestion built (I0–I2). Decode (I3), API (I4), reconciliation (I5),
> deploy (I6) and the clean-clone prove script (I7) are not built.** There is no
> GraphQL endpoint to query today. The [API reference](generated/api.md) is the
> schema the server must implement, not a running service.

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
  through the same normalization path, tagging rows `source='fixture-replay'`.
  Live rows are `source='live'`, and a later write never flips a row's source.

### Retry behaviour

Bounded exponential backoff with jitter on RPC failures. There is no unbounded
retry loop: a failure that exceeds the bound surfaces as an error, and the
cursor does not advance past data that was not stored.

## What is not built

| Milestone | State |
|---|---|
| I3 decode through the pinned Script3 adapter | blocked on the parser WASM event surface (`v0.1.0-alpha.2`) |
| I4 GraphQL (Yoga) implementing the vendored SDL exactly | not started |
| I5 reconciliation of SQL tallies against on-chain reads | not built; `/health` does not exist |
| I6 Docker/compose deploy recipe and a live instance | not built |
| I7 `scripts/prove-indexer` clean-clone proof | not built |

Decoding governor events in TypeScript would put governor logic outside the
parser's `GovernorAdapter` and break the project's rule that governor logic
lives in the parser. That is why I3 waits for a parser release instead of
working around the missing export.

## Checks

| Command | What it proves |
|---|---|
| `npm run check-pins` | vendored bytes match all three locks, offline |
| `npm run typecheck`, `npm run lint` | strict TypeScript and ESLint pass |
| `npm run test:unit` | 48 tests, no database, no network |
| `npm run test:integration` | 11 tests: migrations from empty, windowing, idempotency, restart safety, gaps, replay |

## Provenance

The indexer consumes the parser through pins: the SDL and registry at parser
commit `ba3bf1a3…`, the WASM asset `v0.1.0-alpha.1`. Its README pin table is the
authoritative list; this page summarises it, and the revisions this summary was
written against are recorded in `docs-site/sources.lock`.
