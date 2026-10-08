# Seed v2 — real upstream governors on testnet

Seed v2 replaces the phase-1 self-authored fixture governor with the **actual
pinned upstream governors** (Script3 and OpenZeppelin), deployed on live Stellar
testnet, so the parser is exercised against real governor shapes and real
deployed WASM. Every step is an ordinary signed testnet transaction; every RPC
response used as evidence is captured verbatim in `.seed/capture/` (gitignored)
and curated into committed fixtures by `capture-fixtures.js`.

## Components

| path | role |
|---|---|
| `scripts/upstream/upstream.lock.json` | pins upstream repos to exact SHAs and their built wasm artifacts to sha256 |
| `scripts/upstream/fetch-references.sh` | clones/checks out the pinned SHAs into `references/` (fail closed) |
| `scripts/upstream/build-upstream.sh` | builds the six wasm artifacts and verifies hashes against the lock |
| `scripts/seed-v2/deploy.js` | deploys the six contracts, verifies each deploy tx SUCCESS |
| `scripts/seed-v2/seed.js` | initialize, mint, delegate, propose, vote (idempotent, resumable) |
| `scripts/seed-v2/settle.js` | close/execute matured proposals |
| `scripts/seed-v2/capture-fixtures.js` | curate raw captures into `tests/fixtures/seed-v2/` |
| `scripts/seed-v2/verify.js` | offline integrity check of every recorded hash |
| `scripts/seed-v2/test/` | offline unit tests (Node built-in runner) |
| `scripts/activity/activity.js` | bounded, idempotent on-chain activity generator (keeps fresh data for the indexer) |
| `scripts/ttl/extend-seed-contracts.sh` | extends instance+code TTL of every seed contract read from `.seed/state.json` |

## Root-cause record: Script3 S2 `propose` failed on-chain

**Symptom.** The Script3 proposal `s2-contract-upgrade` (`propose` with an
`Upgrade` action) simulated successfully, was submitted, and its transaction
ended `FAILED` at ledger 5071040. The runner stopped because it treated any
non-SUCCESS status as fatal.

**Diagnosis (independent of the failing SDK path).** Decoded with `stellar xdr
decode` against the raw capture
`0055-getTransaction-final-s3-propose-s2-contract-upgrade.json`:

- `resultXdr` →
  `{"fee_charged":"26607","result":{"tx_failed":[{"op_inner":{"invoke_host_function":"resource_limit_exceeded"}}]},"ext":"v0"}`
- diagnostic event (`budget`) →
  `{"error":{"budget":"exceeded_limit"}}`, data
  `["operation byte-write resources exceeds amount specified", 1240, 1232]`
- the simulation (`0053-…`) declared `resources.write_bytes = 1232`.

So the operation wanted to write **1240** bytes but the transaction declared
**1232**. The contract call itself succeeded (`in_successful_contract_call:
true` and a `proposal_created` event was emitted); the transaction was rejected
purely on the declared-vs-actual resource budget.

**Classification.** Cause **(ii) resources/fees** — not Script3 logic, not an
SDK/XDR vs network-protocol mismatch. (An earlier suspicion of (iii) was an
artifact of a mistyped base64 string; the pinned `@stellar/stellar-sdk 17.2.1`
does decode the result correctly via `xdr.TransactionResult.fromXDR(...).toJSON()`.)

**Fix.** Simulation is a snapshot and the real write footprint can exceed it;
any shortfall fails the whole transaction. Over-declaring resources is always
allowed (you only pay a slightly larger resource fee), so `lib/rpc.js`
(`withResourceMargin`) pads the simulated footprint by
`ceil(value × 1.15) + 64` bytes/instructions before signing, preserving
operations (and simulation-provided auth) via `TransactionBuilder.cloneFrom`.

## Runner resumability

Previously one failed step aborted the whole run with no record. Now
`seed.js`'s `step()`:
- skips steps already in `state.steps`;
- records a failure under `state.failures[name]` with the error and the decoded
  `resultXdr` (`err.decodedResult`, `err.txHash`, `err.ledger`) and retries it on
  the next run;
- supports `SEED_SKIP="<step>,…"` (explicit, recorded with `SEED_SKIP_REASON`);
- supports `SEED_KEEP_GOING=1` to record a failure and continue to later
  independent steps.

## uploadTxHash fix

The deploy wrapper used to scan for "any 64-hex string", which could capture the
CLI's `Deploying contract using wasm hash <sha256>` line as `uploadTxHash` (it
did, for `script3Votes` and `ozMockSubcall`). It now parses only structured
signals: `Signing transaction: <64-hex>` lines and
`stellar.expert/explorer/testnet/tx/<64-hex>` URLs, and treats
`Skipping install because wasm already installed` as "no upload tx". Guarded by
offline tests in `test/cli-parse.test.js` and by `verify.js`.

## Verify

```bash
cd scripts/seed-v2
npm test          # offline parser unit tests
node verify.js    # every recorded hash is a SUCCESS tx, none is a wasm hash
```

## Live findings (testnet, 2026-10-08)

Three live runs were made (process rule 4: two focused attempts per blocker,
labeled fallback next).

**Script3 — complete after a seed redesign.** Script3 enforces *one open
proposal per creator* and a proposal-threshold power requirement. The original
seed cast delegations (delegate-4→delegate-1, delegate-5→delegate-2) *before*
proposing, zeroing the s5/s6 creators' power (`#208`), and the naive "swap
creators" fix tripped the one-open-proposal rule (`#211`). The correct design —

generate proposals FIRST, then cast delegations — is now implemented, and a
clean run created all six proposals (ids 0–5), cast every vote, recorded both
delegations and captured the on-chain reads with **no Script3 failures**.

**OpenZeppelin — blocked on one root cause.** OZ `mint` to `delegate-1/3/5`
traps with `error {storage: exceeded_limit}` — "trying to access contract data
key outside of the footprint" — for key `[TotalSupplyCheckpoint, 1]`: the
transaction's simulated read/write footprint omits a key the mint actually
writes. `mint` to `deployer`/`delegate-2`/`delegate-4` succeeds. Because those
three mints fail, `delegate-1/3/5` hold zero OZ voting power, so the proposals
they create (`o1`/`o3`/`o5`) fail `#5002` (`get_votes_at_checkpoint` = 0) and
their votes fail — i.e. every OZ failure traces to the one footprint defect.
(The self-delegation fix is still required: OZ counts only delegated power.)

So the OZ blocker is **simulation-footprint staleness** — the same *class* as the
Script3 S2 resource issue: the footprint/resources `simulateTransaction` returns
can be stale relative to execution. Re-simulating immediately before signing
and/or routing the affected calls through the CLI's footprint handling is the
understood fix, but it is **not validated live** (two focused attempts used).

## Status as of 2026-10-08

- **Done and verified:** resource-margin fix (S2 validated live), resumable
  runner with decoded-failure records, vote-window skip logic, structured
  `uploadTxHash` parsing + `verify.js` + 4 offline unit tests,
  `capture-fixtures.js` + fixtures README + provenance, `fetch-references.sh`,
  offline wasm-hash claim (6/6), CI org-namespace scoping. Offline claims:
  11 pass / 0 fail / 2 skip. Seed-v2 unit tests: 4 pass.
- **Script3 seed v2: complete** (six proposals, all votes, delegations, reads).
- **OpenZeppelin seed v2: blocked** on the `mint` footprint mismatch above.
- **Still outstanding:** `settle.js` close/execute is implemented and runnable
  (`node settle.js`) but is **not** wired into CI — maturing and executing
  proposals remains a manual operator step.
- **Done since the last live run:** `scripts/activity/activity.js` (bounded,
  idempotent vote generator) and `scripts/ttl/extend-seed-contracts.sh`
  (instance+code TTL extension for every seed contract) exist and are wired
  into the live tier behind a `.seed/state.json` guard, so they are no-ops on
  hosted CI and active only on a self-hosted soak runner. Neither has been
  exercised against live testnet in CI.
- **Seed-v2 exit criterion: partially met** — it re-runs from a clean clone and
  Script3 completes; OZ does not.
