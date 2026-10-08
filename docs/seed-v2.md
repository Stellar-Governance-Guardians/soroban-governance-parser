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

## Status as of 2026-10-08

- **Done:** deploy (7 contracts SUCCESS), Script3 initialize/mint/delegate,
  proposal S1 created. Resource-margin fix, resumable runner, uploadTxHash fix,
  `capture-fixtures.js`, `verify.js`, fixtures README and offline tests all land.
- **Not yet done:** the clean end-to-end re-run on both governors (S2 onward),
  including the OpenZeppelin stack and the `settle.js` close/execute pass; and
  `scripts/activity/` + the live-tier TTL wiring. These require live testnet runs
  and block the seed-v2 exit criterion.
