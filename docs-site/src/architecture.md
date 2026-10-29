# Architecture

Three repositories, one dependency direction, and no shared runtime state beyond
pinned artifacts.

<pre class="mermaid">
flowchart LR
  subgraph chain["Stellar testnet"]
    RPC["Soroban RPC"]
  end

  subgraph parser["soroban-governance-parser (Rust)"]
    CORE["core: ScVal, spec, risk,<br/>GovernorAdapter, simulate"]
    WASM["wasm: wasm-bindgen surface"]
    CLI["cli: sgp (network IO)"]
    CORE --> WASM
    CORE --> CLI
  end

  subgraph indexer["governance-event-indexer (TypeScript)"]
    ING["ingest: windows, cursors,<br/>gaps, replay"]
    DEC["decode: pinned WASM"]
    GQL["GraphQL: governance-v1"]
    ING --> DEC --> GQL
  end

  subgraph dashboard["delegate-portal-dashboard (Next.js)"]
    SIM["/simulate: browser decode"]
    PAGES["pages: indexer GraphQL"]
  end

  RPC -->|getEvents| ING
  CLI -->|getHealth, getEvents,<br/>simulateTransaction| RPC
  WASM -->|.tgz release asset| DEC
  WASM -->|.tgz release asset| SIM
  GQL -->|HTTP| PAGES
  SDL["governance-v1.graphql"] --> GQL
  SDL --> PAGES
</pre>

## Data flow

1. **Chain → parser.** The parser decodes raw `ScVal` values, extracts
   `contractspecv0` from a contract WASM, and applies the published risk rule
   table. Governor-specific decoding lives behind the `GovernorAdapter` trait;
   the Script3 adapter is implemented.
2. **Chain → indexer.** The indexer polls the public RPC's `getEvents` in
   windows of at most 1000 ledgers, stores raw events keyed by
   `(ledger, tx_hash, op_index, event_index)`, and advances one cursor per
   registered contract in the same transaction. Events older than the RPC's
   retention window are recorded in `ingest_gaps` rather than skipped.
3. **Indexer → dashboard.** The dashboard reads the indexer's GraphQL endpoint
   from `INDEXER_URL` and validates every field of every response against the
   vendored SDL. There is no bundled sample dataset and no cached fallback.
4. **Parser → both consumers.** The WASM package and the SDL travel as
   content-pinned artifacts, never as source imports.

## The pin model

A pin is a content hash, not a version range. Each consumer vendors the bytes it
depends on and verifies them before use:

| Consumer | Artifact | Pin file | Checked by |
|---|---|---|---|
| indexer | parser WASM release asset | `parser.lock` | `node scripts/check-pins.mjs` (offline, every PR) |
| indexer | `governance-v1.graphql` | `schema.lock` | same |
| indexer | `deployments.json` | `registry.lock` | same |
| dashboard | parser WASM release asset | `parser.lock` | `npm run check:pins` (offline, every PR) |
| dashboard | `governance-v1.graphql` | `schema.lock` | same |
| this site | sibling docs it summarises | `docs-site/sources.lock` | `docs-site/scripts/check-docs-claims.py` |

Bumping a pin is a PR that shows the diff and re-runs the contract tests. A pin
never points at an asset that does not exist yet: that rule is why the indexer's
decode milestone waits on a real parser release instead of a planned one.

## Why the boundaries sit there

- **Governor logic is parser-only.** Adapters (`matches_event`,
  `normalize_event`, `decode_call`) live behind `GovernorAdapter`. An indexer or
  dashboard that reimplemented them would drift from the parser's golden tests.
- **The parser core does no IO.** `crates/core` is sans-IO; network calls live in
  the CLI (`crates/cli/src/rpc.rs`) and in the indexer's RPC client, so decode
  logic is testable offline.
- **Consumers validate, they do not coerce.** The dashboard rejects a response
  whose fields do not match the SDL instead of filling in defaults.
- **Fail closed, fail loud.** Missing spec, unknown function, unreadable ledger
  range, absent `INDEXER_URL`, broken pin: each becomes an explicit error or
  `UNVERIFIED` value that a reader can see.

## Crates and workspaces

| Path | Role | IO |
|---|---|---|
| `crates/core` | `ScVal` → JSON, WASM custom-section extraction, `contractspecv0`, risk rules, `GovernorAdapter`, power/tally replicas, dry-run model | none |
| `crates/wasm` | wasm-bindgen surface over core for the browser and Node | none |
| `crates/cli` | `sgp` operator tool | Soroban JSON-RPC |
| `governance-event-indexer/src` | ingestion, storage, (planned) decode and GraphQL | Postgres, Soroban RPC |
| `delegate-portal-dashboard/app` | routes; `/simulate` runs in the browser | HTTP to the indexer |
