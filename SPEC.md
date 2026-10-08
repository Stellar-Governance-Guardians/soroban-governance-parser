# SPEC — soroban-governance-parser

Version: v1 (phase 1a + partial phase 1b). Status of every claim is tracked in
`claims.json` and machine-checked by `scripts/check-claims.sh` in CI.

## Purpose
Decode raw Soroban `ScVal` governance payloads (events, proposal calldata,
invocations) into human-readable, risk-classified JSON — governor-agnostically,
fail-closed, with zero mock data in any production path.

## Architecture
Three crates, strict dependency direction (core knows nothing about IO):

| crate | role | IO |
|---|---|---|
| `crates/core` (`soroban-governance-core`) | sans-IO decode: ScVal→JSON, WASM custom-section extraction, `contractspecv0` parsing, risk rules, `GovernorAdapter` trait | none |
| `crates/wasm` (`soroban-governance-wasm`) | wasm-bindgen surface over core for the dashboard | none (pure functions) |
| `crates/cli` (`sgp`) | operator tool + the only place network IO exists (`crates/cli/src/rpc.rs`) | Soroban JSON-RPC |

Downstream repos (`governance-event-indexer`, `delegate-portal-dashboard`)
consume `schemas/` and pin `schema-v1` via their `schema.lock` files.

## Core invariants (non-negotiable)
1. **Fail closed, fail loud.** Every decode path returns `Result`; errors carry
   context. Absent contract spec ⇒ `decoding: "unverified"` + raw ScVal passthrough,
   never invented names. Non-representable constructs (e.g. non-string map keys)
   are errors, not coercions.
2. **Zero mock data in production paths.** Test data is either captured from live
   Stellar testnet (provenance in `tests/fixtures/README.md`) or produced by
   `scripts/seed-testnet/`. CI fails if `crates/*/src` references `tests/fixtures`.
3. **Governor-agnostic.** Governor specifics live only behind `GovernorAdapter`
   (`crates/core/src/adapter.rs`). Verified adapter research:
   `docs/adapters/script3.md`, `docs/adapters/openzeppelin.md`.
4. **Risk tiers are published rules, not verdicts.** Exact-match rule table in
   `crates/core/src/risk.rs`; every classification exposes its matched rule;
   unmatched names are `unverified`, never silently `low`.
5. **Precision safety.** Integers ≥ 64-bit are JSON decimal strings.
6. **Simulation outputs are estimates.** `ExecutionImpact.estimate` is `const true`
   in `schemas/schema-v1.json`; `simulated_at_ledger` is mandatory.

## Claims tiers (process rule 2)
Claims in `claims.json` run in two tiers:
- **PR gate (required, deterministic, offline):** CI runs
  `OFFLINE=1 bash scripts/check-claims.sh` on every PR; testnet claims are
  skipped and reported as SKIP, never PASS.
- **Live tier (not required):** `.github/workflows/live-checks.yml` runs
  `ONLINE_ONLY=1 bash scripts/check-claims.sh` nightly and on manual dispatch;
  on failure it opens or updates a single tracking issue with the re-seed
  command. It never blocks merges.

## Phase 1a scope (delivered)
- Workspace + three crates compiling clean under `clippy::pedantic` (-D warnings).
- ScVal→JSON total converter with fail-closed edge cases (unit-tested, incl. a
  real testnet-captured XDR vector).
- WASM custom-section extractor (LEB128, fail-closed) + `contractspecv0` XDR
  parser (functions/structs/unions/enums/errors/events; total `ScSpecTypeDef`
  renderer).
- `sgp` CLI: `decode-scval`, `spec-from-wasm`, `fetch-spec`, `health`.
- Live-testnet proof: RPC retention window, event capture, spec fetch from a
  real deployed contract, fail-closed demonstration on the non-WASM SAC contract.
- CI: fmt, clippy pedantic, tests, wasm build, fixture-import check, claims check.

## Phase 1b (partial, delivered)
- **Script3 adapter** (`crates/core/src/adapters/script3.rs`) behind
  `GovernorAdapter`: event identification (contract id + symbol + exact arity),
  `normalize_event`, `state_from_chain`, `tally_from_chain`, `decode_proposal`,
  `vote_power_model`. Unknown actions/events fail closed.
- **`RiskContext` contextual rules** (`crates/core/src/risk.rs`):
  `treasury_outflow`, `self_call`, `large_value`, `batched_actions`, each with a
  positive and a negative test.
- **Pure replicas** of the Script3 checkpoint power model
  (`crates/core/src/checkpoint.rs`) and the tally/quorum/outcome rules
  (`crates/core/src/tally.rs`).
- **Offline differential tests** against committed raw captures for proposals
  0–5 (`crates/core/tests/differential.rs`) plus `proptest` property tests
  (`crates/core/tests/proptests.rs`).
- **wasm-bindgen surface** for Script3 decode/state/tally (`crates/wasm`).

## Phase 2 (delivered)
- **WASM package**: `wasm-bindgen` surface built for both `nodejs` and `web`
  (wasm-pack), packaged deterministically into `sgg-parser-wasm-<version>.tgz`
  and published as GitHub **pre-release** `v0.1.0-alpha.1`. The README states
  the sha256; `scripts/check-wasm-release.py` checks it against
  `releases/parser-wasm.lock.json` offline.
- **Schema freeze**: v1 SDL + JSON Schema frozen in `schemas/`, recorded in
  `schemas/CHANGELOG.md`, with an additive-only check
  (`scripts/check-schema-additive.py`) in the PR gate.
- **Node wasm test** (`crates/wasm/js-tests/node.test.mjs`) exercises the built
  `nodejs` package against committed captures.

## Phase 4 (delivered)
- **Sans-IO dry-run** (`crates/core/src/simulate.rs`):
  `build_simulate_request(transaction_xdr_base64, request_id)` builds and
  validates the JSON-RPC request; `parse_simulate_response(json)` produces an
  [`ExecutionImpact`] with `estimate: true` and a mandatory `simulated_at_ledger`.
  Only fields observed in committed real responses are modeled; absent fields are
  `null`. Rent is not modeled (see `docs/dry-run.md`). Tests:
  `crates/core/tests/simulate.rs` over committed captures.

## Out of scope for phase 1a/1b/2/4 (delivered in later phases)
- The **OpenZeppelin adapter** (bounded phase 3; fixtures already captured).
- A CLI `simulate` command (the parser is done; IO wiring is not).
- Delegate metrics (indexer/dashboard phases).

## Ground truths verified in phase 1a (live testnet, 2026-10-05)
- SDF public testnet RPC `https://soroban-testnet.stellar.org`:
  `getHealth` reports `ledgerRetentionWindow: 120960` ledgers; the
  oldest/latest ledger pair drifts with every capture, the window does not.
  Capture: `tests/fixtures/phase1-rpc-getHealth.json`.
- `getEvents` requires `filters` as an ARRAY; event `topic`/`value` arrive as
  base64 XDR strings; wide ledger ranges (>~1000 ledgers) time out on the public
  endpoint — the indexer must page in small windows.
- stellar-xdr 28.0.1 API: flat crate root (no `curr` module), `Limits::none()`,
  `ScSpecTypeDef`, `ScVal::ExecutableTag`, `ScContractInstance{executable,storage}`
  — all verified against the published crate source.
