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

Two focused live re-runs were made after the fixes (process rule 4).

**Validated on-chain**
- The resource-margin fix works: Script3 `propose` for `s2-contract-upgrade`
  succeeded (proposal id 1, ledger 5083482), as did `s3-admin-change` (id 2) and
  `s4-unknown-contract-call` (id 3) and the Script3 votes for them.
- The runner no longer wedges: with the guards in place the run completed with a
  recorded `SEED PARTIAL` failure list instead of aborting.

**Still failing (seed-design defects, exact on-chain errors recorded)**
- **Script3 `s5`/`s6` propose fail** with `Error(Contract, #211)`. #208 (seen
  when s5/s6 kept their original creators) was the proposer-below-threshold
  error; #211 appears once a creator reuses a name that already has an open
  proposal. Script3 enforces **one open proposal per creator** (see
  `upstream.lock.json` `oneOpenProposalPerCreator: true`) and the eight
  power-holders are only `deployer + delegate-1..5`, of which `delegate-4` and
  `delegate-5` delegate their power away *before* proposing. A correct seed must
  either create proposals before casting delegations, or add dedicated proposer
  identities.
- **OpenZeppelin `o2`/`o3`/`o4`/`o6` propose fail** with `Error(Contract, #5002)`
  and `get_votes_at_checkpoint` returning `0` for the proposer. OZ only counts
  **delegated** voting power (no self-votes without a delegation), and the seed
  only delegates `delegate-4→delegate-1` / `delegate-5→delegate-2`. Every OZ
  proposer/voter must delegate (including to self) before voting power is live.
- **OZ mints to `delegate-1`/`3`/`5` fail** with `invoke_host_function: trapped`
  (the other four mints succeed); needs a focused diagnosis.
- **Script3 `S1` votes cannot be cast on a resume** whose delay exceeds the vote
  period (`vote_end 5071759` < current ledger); the runner now records this as a
  reasoned skip rather than a failure.

## Status as of 2026-10-08

- **Done and verified:** resource-margin fix (validated live: S2 on-chain),
  resumable runner with decoded-failure records, vote-window aware skip logic,
  structured `uploadTxHash` parsing + `verify.js` + offline unit tests,
  `capture-fixtures.js` + fixtures README + provenance, `fetch-references.sh`,
  offline wasm-hash claim (6/6), CI org-namespace scoping fix. Offline claims:
  11 pass / 0 fail / 2 skip. Seed-v2 unit tests: 4 pass.
- **Not met (seed-v2 exit criterion):** a clean end-to-end run on both
  governors. Blocked by the seed-design defects above; the fixes (delegations
  after proposals + a dedicated proposer identity; OZ self-delegation) are
  understood but **not yet validated live**. `settle.js` close/execute, the
  `scripts/activity/` scheduler and the live-tier TTL wiring remain outstanding.
