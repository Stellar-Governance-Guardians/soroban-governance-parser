# soroban-governance-parser

Rust workspace that decodes raw Soroban `ScVal` governance payloads (events,
proposal calldata, invocations) into human-readable, risk-classified JSON.
Governor-agnostic, fail-closed, zero mock data in production paths.

Part of the Stellar-Governance-Guardians suite:
[parser](https://github.com/Stellar-Governance-Guardians/soroban-governance-parser) →
[indexer](https://github.com/Stellar-Governance-Guardians/governance-event-indexer) →
[dashboard](https://github.com/Stellar-Governance-Guardians/delegate-portal-dashboard).

Phase status: the merged work in this repository is **Phase 1a**. Parser
completion — seed v2 from pinned upstream SHAs, Script3/OpenZeppelin adapters,
`RiskContext`, vote-power/tally replicas, dry-run modeling, wasm-pack packaging,
differential tests — is **Phase 1b** and is not yet complete.

## What it does (phase 1a)
- **ScVal → JSON**: total converter over the stellar-xdr 28 type set. Integers
  ≥ 64-bit become decimal strings (no precision loss). Non-representable
  constructs (e.g. non-string map keys) are errors, never coercions.
- **contractspecv0 extraction**: minimal fail-closed WASM custom-section parser
  + XDR spec parser (functions, structs, unions, enums, errors, events).
- **Argument decoding with verified names**: positional `ScVal` args are zipped
  with declared input names from the contract's own spec. Arity mismatch or a
  missing spec ⇒ explicit error / `decoding: "unverified"`, never guesses.
- **Risk classification**: exact-match published rule table
  (`crates/core/src/risk.rs`). Every classification exposes the rule that
  matched. Unknown functions are `unverified`, never silently `low`.
- **`GovernorAdapter` trait**: governor-specific decoding lands behind this
  trait (adapters for Script3 + OpenZeppelin governors are phase 1b; verified
  research with sources is already in `docs/adapters/`).

## Crates
| crate | purpose | IO |
|---|---|---|
| `crates/core` | sans-IO decode pipeline | none |
| `crates/wasm` | wasm-bindgen bindings for the dashboard | none |
| `crates/cli` (`sgp`) | operator CLI; the only network code (`src/rpc.rs`) | Soroban JSON-RPC |

## Quick start
```bash
cargo build --release -p soroban-governance-cli

# RPC health / retention window
./target/release/sgp health

# Decode any ScVal (base64 XDR)
./target/release/sgp decode-scval AAAADwAAABBwcm9wb3NhbF9jcmVhdGVk
# => "proposal_created"

# Fetch a live contract's WASM and parse its contractspecv0
./target/release/sgp fetch-spec --contract CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE

# Parse a local WASM
./target/release/sgp spec-from-wasm path/to/contract.wasm

# Protocol 29 TTL: instance + code liveUntilLedger (fail closed if missing)
./target/release/sgp ttl --contract CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE
# Nightly-live-tier gate form: exit 1 if fewer than N ledgers remain
./target/release/sgp ttl --contract CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE --min-remaining 50000
```

## Verified evidence (live Stellar testnet, 2026-10-05)
All items below are machine-checked by `scripts/check-claims.sh` (see
`claims.json`) and reproducible via `scripts/prove-phase1.sh`
(output: `tests/fixtures/phase1-proof.json`, provenance:
`tests/fixtures/README.md`).

| fact | evidence |
|---|---|
| SDF public testnet RPC retention window = 120,960 ledgers | `sgp health` vs deployments.json; getHealth capture in fixtures |
| contractspecv0 parse works on a real deployed contract | fixture governor `CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE` (deploy tx `a37b875d…`, ledger 5035901); spec functions `propose/vote/get_proposal` parsed from live WASM |
| real governance events decode correctly | `proposal_created` tx `34dd7cef…` ledger 5035906, `vote_cast` tx `ddef3540…` ledger 5035908 — topics/values decoded to `("Fund parser audit", u128 1000000)` and `(1, u128 1000000)` |
| fail-closed on non-WASM contracts | `sgp fetch-spec` on the testnet SAC contract exits non-zero with "contract is not WASM-backed … no contractspecv0 available" |
| 23 unit tests pass; clippy pedantic `-D warnings` clean | `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` |
| stellar-xdr pinned 28.0.1 | `Cargo.lock` |

## Honest limitations
- **No concrete governor adapters yet.** The trait and verified research exist
  (docs/adapters/); Script3 and OpenZeppelin adapters are phase 1b. Until then,
  governor attribution is always `unverified` — by design.
- Event shapes in `docs/adapters/` were read from upstream source, not yet
  observed by us on live deployments (neither project publishes testnet IDs).
  The fixture contract emits Script3-shaped topics, but a fixture is not the
  real governor.
- The public testnet RPC times out on wide `getEvents` ranges (>~1000 ledgers);
  consumers must page in small windows. Retention is ~7 days — historical
  backfill (Galexie/Hubble) is an indexer-phase concern.
- `simulateTransaction` is NOT yet modeled. `ExecutionImpact` exists in the
  schema with `estimate: true` const, but no producer until the live response
  shape is inspected (phase 1b/2).
- Vote-weight decay: UNVERIFIED on both governors; no claims made.
- The fixture deployer key is a throwaway testnet key; its secret is not in
  this repo. The fixture contract holds no value and is not a governor.

## Layout
```
crates/core      sans-IO decode pipeline (ScVal, WASM, spec, risk, adapters)
crates/wasm      wasm-bindgen surface
crates/cli       sgp binary + RPC client (only IO layer)
schemas/         cross-repo source of truth (schema-v1.json, governance-v1.graphql)
docs/adapters/   verified per-governor research with source URLs
scripts/         prove-phase1.sh, check-claims.sh, check-proof.py, seed-testnet/ (fixture governor),
                 ttl/extend-fixture-ttl.sh (idempotent TTL extension)
tests/fixtures/  live-captured testnet data + provenance README
claims.json      machine-checkable claims ledger (charter rule 8)
deployments.json deployment registry (real tx hashes and ledgers only)
```

## Development
```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
scripts/check-claims.sh     # all claims; OFFLINE=1 (PR gate) or ONLINE_ONLY=1 (live tier)
scripts/check-proof.py      # verify the recorded proof supports the evidence table
scripts/prove-phase1.sh     # live testnet proof (needs network)
```
Rust toolchain: pinned via `rust-toolchain.toml`. Contract fixture builds with
`stellar contract build` (wasm32v1-none — soroban-sdk 28 rejects
wasm32-unknown-unknown on Rust ≥ 1.82).

## License
MIT — see [LICENSE](LICENSE).
