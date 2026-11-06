# Indexer quickstart

Node ≥ 24, Postgres 16, and the vendored pinned artifacts (committed).

## Run the checks

```bash
git clone https://github.com/Stellar-Governance-Guardians/governance-event-indexer
cd governance-event-indexer
npm ci

docker compose up -d postgres
export DATABASE_URL='postgres://indexer:indexer-local-only@localhost:5432/indexer'

npm run check-pins        # sha256 of the vendored WASM, SDL and registry
npm run typecheck
npm run lint
npm run test:unit         # 48 tests, no database, no network
npm run test:integration  # 11 tests against Postgres
npm run build
```

The integration suite creates a throwaway database next to `DATABASE_URL` and
drops it, so "migrations from empty" is exercised every run. Without
`DATABASE_URL` it fails with an explicit message instead of skipping.

## Apply migrations and ingest

```bash
node dist/cli.js migrate
node dist/cli.js ingest     # polls getEvents in ≤1000-ledger windows
node dist/cli.js replay <dir>   # replays committed raw responses, source='fixture-replay'
```

## What exists and what does not

Built: ingestion (I2) — registry, cursors, windowed `getEvents`, idempotent raw
storage, retention-gap recording, fixture replay, durable parser-backed event
decode, and an initial GraphQL health/raw-event connection.

Not built: proposal/impact/delegate projections, reconciliation (I5), deploy
(I6) and the clean-clone prove script (I7).
The [API reference](../generated/api.md) describes the pinned schema and the
implemented health/event surface.

The remaining decode milestone depends on persisted tables and replay semantics,
not on a missing parser export; see [Honest limitations](../limitations.md).

## Pins

| Pin | Artifact |
|---|---|
| `parser.lock` | WASM release asset `v0.1.0-alpha.2` (sha256 `65193e43…`) |
| `schema.lock` | `governance-v1.graphql` @ parser `ba3bf1a3…` (sha256 `cfbd85a8…`) |
| `registry.lock` | `deployments.json` @ the same parser commit |

The PR gate verifies all three offline; the nightly `nightly-pins` workflow
checks that the pinned URLs still resolve to identical bytes.
