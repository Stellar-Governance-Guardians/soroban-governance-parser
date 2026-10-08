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
| real upstream governors live on testnet | seed v2 deployed pinned Script3 + OpenZeppelin governors and captured 232 raw RPC fixtures (ledgers 5083606–5084619); `docs/seed-v2.md`, `tests/fixtures/seed-v2/` |
| stellar-xdr pinned 28.0.1 | `Cargo.lock` |

## Honest limitations
- **No concrete governor adapters yet.** The trait and verified research exist
  (docs/adapters/); Script3 and OpenZeppelin adapters are phase 1b. Until then,
  governor attribution is always `unverified` — by design.
- Event shapes in `docs/adapters/` were originally read from upstream source
  rather than observed on live deployments. **That changed with seed v2**: the
  pinned upstream Script3 and OpenZeppelin governors are now deployed on live
  testnet and their real proposals, votes and delegations are captured as raw
  RPC fixtures under `tests/fixtures/seed-v2/`. See
  [`docs/seed-v2.md`](docs/seed-v2.md). These are testnet deployments of the
  pinned upstream commits, **not** mainnet DAOs — no mainnet deployment of
  either governor is claimed.
- The public testnet RPC times out on wide `getEvents` ranges (>~1000 ledgers);
  consumers must page in small windows. Retention is ~7 days — historical
  backfill (Galexie/Hubble) is an indexer-phase concern.
- `simulateTransaction` is NOT yet modeled. `ExecutionImpact` exists in the
  schema with `estimate: true` const, but no producer until the live response
  shape is inspected (phase 1b/2).
- Vote-weight decay: **verified absent in both governor sources** (2026-10-08).
  Script3 derives power from `get_past_votes` checkpoint lookups
  (`references/soroban-governor/contracts/votes/src/checkpoints.rs`) and the
  OpenZeppelin stack from `get_votes_at_checkpoint`
  (`references/stellar-contracts/packages/governance/src/votes/storage.rs`):
  both are snapshot reads of stored balances with no time-decay term anywhere
  in their vote-accounting code. No decay is implemented in this parser,
  because neither governor implements it.
- The fixture deployer key is a throwaway testnet key; its secret is not in
  this repo. The fixture contract holds no value and is not a governor.

## Testnet contracts can vanish; the fixtures are the evidence

The contract ids in `deployments.json` were real on public testnet on the date
recorded next to each entry. **Testnet is periodically reset and contracts have a
TTL, so those ids can stop resolving at any time without warning.** A reset
typically leaves the ids *addressable but empty*, because a contract id is
derived from the deploy salt rather than the ledger history.

That is exactly why every claim in this repo is backed by a **committed raw RPC
response** under `tests/fixtures/`, not by a live query:

- Offline tests, claims and the parser must keep working with the network
  disabled. They read the fixtures, never testnet.
- `deployments.json` is not trusted on its own:
  `scripts/check-deployments.py` re-derives every registered entry from the
  committed fixtures and the pinned `upstream.lock.json`, and runs in the PR
  gate.
- The nightly live tier only *reports* on TTL; it never blocks a merge.

If a registered contract has expired or a reset has wiped it, re-seed rather than
hand-editing the registry:

```bash
scripts/upstream/fetch-references.sh          # pinned upstream SHAs into references/
scripts/upstream/build-upstream.sh            # rebuild + verify pinned wasm hashes
cd scripts/seed-v2 && npm ci
node deploy.js && node seed.js && node settle.js
node snapshot-power.js                    # power-at-snapshot reads (N3 evidence)
node capture-fixtures.js                      # refresh committed fixtures + index.json
node verify.js                                # offline integrity of recorded hashes
python3 ../../scripts/check-deployments.py    # registry must match the new fixtures
```

If the contracts are merely **expired but still present**, extend their TTL
instead of re-deploying — this is idempotent and skips healthy entries:

```bash
bash scripts/ttl/extend-seed-contracts.sh      # reads ids from .seed/state.json
```

**TTL extension is not run in CI.** It needs a funded signer, so it only runs on
the self-hosted soak runner that carries the gitignored `.seed/` working area
(where the keys live). Hosted CI, including the nightly live tier, only *reads*
TTL over public RPC — reading TTL needs no secrets at all. That split is
deliberate: CI can never acquire the authority to sign.

## Layout
```
crates/core      sans-IO decode pipeline (ScVal, WASM, spec, risk, adapters)
crates/wasm      wasm-bindgen surface
crates/cli       sgp binary + RPC client (only IO layer)
schemas/         cross-repo source of truth (schema-v1.json, governance-v1.graphql)
docs/adapters/   verified per-governor research with source URLs
scripts/         prove-phase1.sh, check-claims.sh, check-proof.py, check-deployments.py,
                 list-registered-contracts.sh, seed-testnet/ (fixture governor),
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
