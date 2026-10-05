# tests/fixtures — provenance ledger

Charter rule 1: every fixture here is either (a) captured from live Stellar
testnet with recorded provenance, or (b) produced by `scripts/seed-testnet/`.
No hand-written mock data. CI fails if any `crates/*/src` file references this
directory.

## Live testnet captures (all 2026-10-05, SDF public testnet RPC
## `https://soroban-testnet.stellar.org`)

| file | what | provenance |
|---|---|---|
| `phase1-rpc-getHealth.json` | getHealth response; retention window 120960 | captured at latestLedger 5035284 |
| `phase1-rpc-getEvents-sac.json` | getEvents sample (80-ledger window, contract filter) | SAC contract `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`, first event: ledger 5035200, tx `2dceb6574b49e4d4c6f3...` |
| `phase1-fixture-events.json` | the two events emitted by our deployed fixture governor | contract `CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE`; `proposal_created` ledger 5035906 tx `34dd7cef8df6234f381563fff88b27a1163557f344f39832ff0e9901711215e3`; `vote_cast` ledger 5035908 tx `ddef35404ed2fbef74c1e2324af631398aed9949ff2c7586d56d7cb17bfbea38` |
| `phase1-proof.json` | full machine output of `scripts/prove-phase1.sh` | live captures of 2026-10-05, assembled with the same logic as the script (below); regenerate with the reproduce command |

How `phase1-proof.json` was produced (2026-10-05): `sgp fetch-spec` was run
against the deployed fixture governor (live RPC, spec read at ledger 5036690),
every captured event `topic`/`value` was decoded with `sgp decode-scval`, and
the negative proof re-ran `sgp fetch-spec` against the testnet SAC contract
(exit 1). The `toolchain` block records the last full local run observed the
same day (23 tests passing, `cargo clippy --workspace --all-targets -D warnings`
clean) and the sha256 of the built parser wasm. `scripts/prove-phase1.sh`
reproduces all of it end-to-end; `scripts/check-proof.py` fails loudly if any
field ever stops supporting the README evidence table.

## Fixture governor deployment (produced by scripts/seed-testnet/)

- Source crate: `scripts/seed-testnet/` (soroban-sdk 28.0.0, built with
  `stellar contract build`, target wasm32v1-none — soroban-sdk 28 rejects
  wasm32-unknown-unknown on Rust ≥ 1.82).
- WASM sha256: `a54dd248e58f0fa945c718ac3badfa03dbaa1642d2a08dc409caeaf3d3555b38` (2951 bytes)
- Deployer (testnet, friendbot-funded): `GAMARWFU4EN24QSZWDYMTKK4FIRA7WGPPXH57PPWPUNVRXWNEYEAUP3C`
  - funding tx: `cd82c803a21ad1146edbdfb05187665217ba81d1f79940f020ed2100cf0cb677` (ledger 5035896)
  - wasm upload tx: `5b6fc833ed2120cd1616bedfb5be5ba91d8d2b5224c9d2fd52b5cf62b145f03d` (ledger 5035899)
  - contract create tx: `a37b875db6c67674801f604c6a0dea1194cea6ab205bfa5252d55cecdb6d9bda` (ledger 5035901)
- Contract id: `CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE`
- Invocations: `propose(creator, "Fund parser audit", 1000000) -> 1` (tx `34dd7cef...`, ledger 5035906);
  `vote(voter, 1, 1)` (tx `ddef3540...`, ledger 5035908).

The deployer key is a throwaway testnet key with no mainnet funds; its secret
stays out of this repo (charter rule 6 — no secrets in repos).

## Reproduce

```bash
scripts/prove-phase1.sh          # regenerates phase1-proof.json against live testnet
scripts/check-claims.sh          # verifies every README/SPEC claim
```

Note: testnet is periodically reset. If the fixture contract has been wiped,
re-run the deployment steps in `scripts/prove-phase1.sh` header comments and
update `deployments.json` in the same PR.
