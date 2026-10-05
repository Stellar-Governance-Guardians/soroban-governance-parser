# SPEC — soroban-governance-parser

Version: v1 (phase 1). Status of every claim is tracked in `claims.json` and
machine-checked by `scripts/check-claims.sh` in CI.

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

## Phase 1 scope (this revision)
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

## Out of scope for phase 1 (next phases)
- Concrete `GovernorAdapter` impls for Script3 and OZ governors (phase 2;
  research already recorded with sources in `docs/adapters/`).
- `simulateTransaction`-based `ExecutionImpact` producer (phase 2/3; response
  shape must be inspected live before modeling — rule: verify first).
- Vote-weight decay, delegate metrics (indexer/dashboard phases).

## Ground truths verified in phase 1 (live testnet, 2026-10-05)
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
