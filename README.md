# soroban-governance-parser

Rust workspace that decodes raw Soroban `ScVal` governance payloads (events,
proposal calldata, invocations) into human-readable, risk-classified JSON.
Governor-agnostic, fail-closed, zero mock data in production paths.

Part of the Stellar-Governance-Guardians suite:
[parser](https://github.com/Stellar-Governance-Guardians/soroban-governance-parser) →
[indexer](https://github.com/Stellar-Governance-Guardians/governance-event-indexer) →
[dashboard](https://github.com/Stellar-Governance-Guardians/delegate-portal-dashboard).

Phase status: the merged work covers **Phase 1a, Phase 1b (Script3), Phase 2
(WASM package + schema freeze) and Phase 4 (dry-run modeling)**. Seed v2 from
pinned upstream SHAs, the `Script3Adapter`, `RiskContext`, the
vote-power/checkpoint and tally/quorum replicas, the offline differential tests,
the published WASM package and the sans-IO dry-run parser are done. Pending: the
OpenZeppelin adapter (bounded phase 3) and a CLI `simulate` command. Evidence
below.

## What it does
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
  trait. The **Script3 adapter is implemented**
  (`crates/core/src/adapters/script3.rs`): it identifies the pinned governor's
  events by contract id + symbol + exact topic arity, decodes
  `proposal_created`/`vote_cast`/`proposal_voting_closed`/…, reconstructs
  proposal state and tally from `get_proposal`/`get_proposal_votes`, and decodes
  the five `ProposalAction` variants (unknown variants are preserved, never
  guessed). The **OpenZeppelin adapter is not implemented yet**; its research
  and fixtures exist (`docs/adapters/openzeppelin.md`,
  `tests/fixtures/seed-v2/`).
- **Risk rules are data, not verdicts**: exact-match function-name rules
  (`crates/core/src/risk.rs`, positive + negative test per rule) plus contextual
  rules — `treasury_outflow`, `self_call`, `large_value`, `batched_actions` —
  that evaluate the decoded arguments and emit every fired rule with its
  evidence. No opaque composite score.
- **Power/tally replicas**: pure, IO-free replicas of the pinned Script3
  checkpoint power model (`crates/core/src/checkpoint.rs`) and the tally /
  quorum / outcome rules (`crates/core/src/tally.rs`), differentially tested
  against committed on-chain reads.

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

## WASM package

The parser exposes its decode / risk / tally surface through `wasm-bindgen` for
both the `nodejs` and `web` targets, packaged in one archive for the dashboard.

| | |
|---|---|
| release | GitHub **pre-release** `v0.1.0-alpha.1` (not on npm or crates.io) |
| asset | `sgg-parser-wasm-0.1.0-alpha.1.tgz` |
| sha256 | `7b2eb9420f0cf0582d079f9603beb1e8548e1c7bac165724688cbae5b9237ecd` |
| lock | [`releases/parser-wasm.lock.json`](releases/parser-wasm.lock.json) |

Rebuild it deterministically (sorted tar entries, zeroed mtimes, `gzip -n`), so
the sha256 is stable:

```bash
scripts/wasm/package-release.sh 0.1.0-alpha.1
sha256sum sgg-parser-wasm-0.1.0-alpha.1.tgz   # must equal the lock file
```

`scripts/check-wasm-release.py` (a PR-gate claim) checks that the README's
sha256 and asset name match the committed lock file, offline. Sibling repos pin
this lock file per the interface contract.

## Schemas (v1, frozen)

`schemas/governance-v1.graphql` (GraphQL SDL) and `schemas/schema-v1.json`
(JSON Schema) are the cross-repo contract. v1 is **additive-only**: see
[`schemas/CHANGELOG.md`](schemas/CHANGELOG.md), enforced by
`scripts/check-schema-additive.py` against the committed baseline
`schemas/governance-v1.baseline.graphql` (a PR-gate claim). Sibling repos pin
the SDL together with `releases/parser-wasm.lock.json`.

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
| 85 tests pass (unit + differential + proptest + dry-run); clippy pedantic `-D warnings` clean | `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` |
| the Script3 adapter reproduces real chain state | differential tests decode committed `simulateTransaction` captures for proposals 0–5 (`get_proposal`, `get_proposal_votes`, `get_past_votes`, `get_past_total_supply`) and assert equality; `crates/core/tests/differential.rs` |
| real upstream governors live on testnet | seed v2 deployed pinned Script3 + OpenZeppelin governors and captured 259 raw RPC fixtures (ledgers 5083606–5086043); `docs/seed-v2.md`, `tests/fixtures/seed-v2/` |
| OpenZeppelin fixtures captured; adapter pending | six OZ proposal shapes exist in the corpus (`o1`…`o6`); `o1`/`o3` carry votes, `o2`/`o4`/`o6` do not (their vote windows closed before the checkpoint fix). The OZ adapter is not yet implemented |
| dry-run models real `simulateTransaction` responses | `crates/core/src/simulate.rs` + `crates/core/tests/simulate.rs` parse committed captures: `estimate:true`, `simulated_at_ledger`, `min_resource_fee_stroops`; `cost` was never observed so `cpu_instructions`/`memory_bytes` are `null` |
| stellar-xdr pinned 28.0.1 | `Cargo.lock` |

## Honest limitations
- **Script3 adapter only; OpenZeppelin is not implemented.** Governor
  attribution for any contract that is not the pinned Script3 governor stays
  `unverified` — by design.
- **Differential coverage is Script3 proposals 0–5 only**, from a single seeded
  deployment of each governor — not other deployments, not mainnet DAOs. The
  corpus contains no `Against` votes, and OZ proposals `o2`/`o4`/`o6` carry no
  votes.
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
- **Dry-run is modeled but not CLI-exposed.** `parse_simulate_response` reads
  the fields observed in committed `simulateTransaction` captures; the CLI has
  no `simulate` command yet, so it is exercised by offline tests only. Rent is
  not modeled ([`docs/dry-run.md`](docs/dry-run.md)).
- Vote-weight decay: **not implemented here.** A source reading of both pinned
  governors — Script3 `contracts/votes/src/checkpoints.rs`
  (`get_past_votes`) and OpenZeppelin
  `packages/governance/src/votes/storage.rs` (`get_votes_at_checkpoint`), at the
  SHAs pinned in `scripts/upstream/upstream.lock.json` — found no time-decay
  term: power is a stored checkpoint read. This is a **source reading, not a
  live-verified claim** (reproduce with `scripts/upstream/fetch-references.sh`
  plus a full-text search), so no decay is modeled.
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

## Codespaces / dev container

Open in Codespaces: <https://codespaces.new/Stellar-Governance-Guardians/soroban-governance-parser>

`.devcontainer/` provisions Rust 1.96 (per `rust-toolchain.toml`) with both wasm
targets, Node 24, Docker-in-Docker and the GitHub CLI, and
`.devcontainer/post-create.sh` adds `wasm-pack`, `gitleaks` and `stellar-cli`.

- **Secrets never live in the repo.** Funded testnet keys for the seed-v2
  workflow and any DB URLs come only from Codespaces secrets / env vars.
  `.seed/` and `.env*` are gitignored.
- Run `gitleaks detect --source .` before every push.
- Codespaces sleep when idle, so do **not** run anything that must stay up
  (the seed-v2 soak/activity loop) inside one; host it on a VPS instead.

## Layout
```
crates/core      sans-IO decode pipeline (ScVal, WASM, spec, risk, adapters)
crates/wasm      wasm-bindgen surface
crates/cli       sgp binary + RPC client (only IO layer)
schemas/         cross-repo source of truth (schema-v1.json, governance-v1.graphql)
docs/adapters/   per-governor research + source-to-behavior mapping
                 (Script3 implemented; OpenZeppelin pending)
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
