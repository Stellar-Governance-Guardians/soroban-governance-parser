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
| `test/footprint-margin.test.js` | offline tests for the checkpoint-footprint widening, driven from committed fixtures |

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

Four live runs were made. The first three resolved the Script3 seed design; the
fourth resolved the OpenZeppelin blocker on its first focused attempt, after a
decode-and-compare pass established the real root cause instead of the assumed
one (process rule 4: test the hypothesis, do not assume it).

**Script3 — complete after a seed redesign.** Script3 enforces *one open
proposal per creator* and a proposal-threshold power requirement. The original
seed cast delegations (delegate-4→delegate-1, delegate-5→delegate-2) *before*
proposing, zeroing the s5/s6 creators' power (`#208`), and the naive "swap
creators" fix tripped the one-open-proposal rule (`#211`). The correct design —

generate proposals FIRST, then cast delegations — is now implemented, and a
clean run created all six proposals (ids 0–5), cast every vote, recorded both
delegations and captured the on-chain reads with **no Script3 failures**.

**OpenZeppelin — root cause found and fixed.** See the dedicated section below.

## Root cause: the OpenZeppelin checkpoint index (fixed, validated live)

**Symptom.** `mint` to `delegate-1/3/5` trapped with
`error {storage: exceeded_limit}` — "trying to access contract data key outside
of the footprint" — for key `[TotalSupplyCheckpoint, 1]`, while the submitted
footprint declared only `[TotalSupplyCheckpoint, 0]`. `mint` to
`deployer`/`delegate-2`/`delegate-4` succeeded.

**The obvious reading was wrong.** The obvious diagnosis is "the simulated
footprint is stale relative to execution", and the submitted transaction's
footprint appears to confirm it. Decoding the captured simulation's
`transactionData` and the submitted `envelopeXdr` side by side shows they were
**identical** — both declared `[TotalSupplyCheckpoint, 0]`. Nothing went stale
between simulation and signing.

**What actually differs is the ledger the simulation ran against.** `invoke()`
broke out of its `getTransaction` wait loop as soon as a status came back, and
that resolves when a transaction is *included* in a ledger, which is **before
that ledger closes**. So the next operation simulated against a snapshot that
did not yet include the previous transaction's state changes.

That matters because OpenZeppelin's votes storage reads its checkpoint counter
from **instance** storage and writes the entry to **persistent** storage at index
`num` (`references/stellar-contracts` @ `b40c5eaefe6a29f0030f00bd2d730b7a91cce330`,
`packages/governance/src/votes/storage.rs`, `push_checkpoint` / `get_num_checkpoints`):

```rust
let num = get_num_checkpoints(e, checkpoint_type);      // instance storage
let last_checkpoint = if num > 0 { Some(get_checkpoint(e, checkpoint_type, num - 1)) } else { None };
if last_checkpoint.ledger == e.ledger().sequence() { /* update num-1 in place */ }
let key = checkpoint_storage_key(checkpoint_type, num);   // else write a NEW one
```

One successful mint in between increments `num`, so the index the simulation
predicted is exactly one lower than execution needs.

**The failures were a strict alternation**, not noise — every mint whose
immediately-preceding mint succeeded failed:

| tx | sim ledger | declared index | exec ledger | result |
|---|---|---|---|---|
| `oz-mint-sgg-deployer` | 5083652 | `[0]` | 5083653 | SUCCESS |
| `oz-mint-sgg-delegate-1` | 5083653 | `[0]` | 5083654 | **FAILED** (wanted `[1]`) |
| `oz-mint-sgg-delegate-2` | 5083654 | `[1]` | 5083655 | SUCCESS |
| `oz-mint-sgg-delegate-3` | 5083655 | `[1]` | 5083656 | **FAILED** (wanted `[2]`) |
| `oz-mint-sgg-delegate-4` | 5083656 | `[2]` | 5083657 | SUCCESS |
| `oz-mint-sgg-delegate-5` | 5083657 | `[2]` | 5083658 | **FAILED** (wanted `[3]`) |

**Fix 1 (the one that mattered).** `waitForLedgerClose()` in `lib/rpc.js` polls
`getHealth` until `latestLedger` exceeds the landed ledger, so the next
simulation observes at least the state execution will. It fails closed rather
than continuing on a snapshot known to be stale.

**Fix 2 (defence in depth).** `widenCheckpointFootprint()` also declares the
next `CHECKPOINT_INDEX_MARGIN` indices above the highest one the simulation
predicted, anchored to that highest index. Over-declaring footprint keys is
always legal — the contract only touches what it needs, and resource fees are
charged on what is actually read and written — so this is free when unused. It
is a no-op for governors that keep no such counter (asserted offline against a
captured Script3 simulation).

**Validated live.** The same mints, before and after:

| | old | new |
|---|---|---|
| `delegate-1` | FAILED, ledger 5083654, `803d0bc41e1252603c78ae714080f32797b3751032582f19db3ce9ce0651d59e`, declared `[0]` | SUCCESS, ledger 5084551, `01333a94a65808002409a9ed3f435f42…`, declared `[4, 5, 6]` |
| `delegate-3` | FAILED, ledger 5083656, `2aef69bdbca83ad47341d9c604cd41ca70fb2bfef62a8bcb18037531d6906945` | SUCCESS, ledger 5084553, `949a34b99dec07f4068c875f` |
| `delegate-5` | FAILED, ledger 5083658, `e352ae22bf264bbc3f80af3f8d9c44431ffdef1938374898777bb6f7a94c2b60` | SUCCESS, ledger 5084555, `913270d1c1681af3342de725` |

Both fixture sets are committed; the FAILED transactions were kept rather than
overwritten so the contrast stays legible.

**One further bug surfaced only once the mints worked.** The OZ vote path
passed the hex32 proposal id to `scAddress()`, which requires a `G…`/`C…`
address, so every vote threw `Unsupported address type` before reaching the
chain. The pinned wasm's contractspec confirms
`cast_vote(proposal_id: BytesN<32>, vote_type: u32, reason: String, voter: Address)`;
the adjacent `proposal_state` read already used `scBytes32`. Fixed.

## Registered contracts and their TTL

All seven seed-v2 contracts (plus the phase 1 fixture governor) are now
registered in `deployments.json` with their contract id, wasm sha256, wasm file,
deploy tx hash, deploy ledger, upstream repo, upstream commit SHA and license.
`scripts/check-deployments.py` re-derives every one of those fields from the
committed fixtures and the pinned `scripts/upstream/upstream.lock.json`, and runs
as an offline claim in the PR gate, so the registry cannot silently drift from
the evidence.

`ozMockSubcall` reuses the Script3 mock-subcall wasm, which is why two
registered contracts share one wasm sha256.

Live TTL snapshot, read over public RPC (no secrets) at ledger ~5085000:

| contract | role | instance ledgers left | code ledgers left |
|---|---|---|---|
| `fixtureGovernor` | `CDJWPKSQ4N…` | 3074927 | 3074930 |
| `seedV2OpenZeppelin.ozGovernor` | `CAONSV2R2Y…` | 119738 | 106900 |
| `seedV2OpenZeppelin.ozMockSubcall` | `CAXHKR66BE…` | 119741 | 521873 |
| `seedV2OpenZeppelin.ozToken` | `CA77UOHZIP…` | 119737 | 106899 |
| `seedV2OpenZeppelin.ozUpgradeableV1` | `CC3SMQABLU…` | 119739 | 106901 |
| `seedV2Script3.script3Governor` | `CCKGJBCBBY…` | 776387 | 763791 |
| `seedV2Script3.script3MockSubcall` | `CAQCXFI6YS…` | 534470 | 521874 |
| `seedV2Script3.script3Votes` | `CCAUJK6V6G…` | 534466 | 521870 |

Note the two tiers: the Script3 contracts carry a much longer TTL than the
OpenZeppelin ones, so the OZ contracts are what the live tier will warn about
first. The live tier **reads** TTL for every registered contract and files a
tracking issue on failure; it never blocks a merge. TTL **extension** needs a
funded signer and therefore runs only on the self-hosted soak runner that holds
the gitignored `.seed/` keys (`scripts/ttl/extend-seed-contracts.sh`). Hosted CI
can never acquire signing authority.

Contracts on public testnet can disappear on a reset and expire via TTL, so
`deployments.json` is a record of what was real, not a promise that it still
resolves. The committed fixtures under `tests/fixtures/seed-v2/` are the durable
evidence; see "Honest limitations" in the root README for the re-seed command.

## Status as of 2026-10-08

- **Script3 seed v2: complete.** Six proposals (ids 0–5), 10 votes including 3
  abstains on `s6-abstain-heavy`, both delegations, and `get_proposal` /
  `get_proposal_votes` reads captured for every proposal.
- **OpenZeppelin seed v2: unblocked, and now six proposals too.** All six shapes
  exist: `o1` transfer-executed, `o2` contract-upgrade, `o3` admin-change,
  `o4` unknown-contract-call, `o5` failing-quorum, `o6` abstain-heavy.
  `o1` and `o3` carry votes; `o5` has none by design.
- **Done and verified offline:** resource-margin fix (S2), resumable runner
  with decoded-failure records, vote-window skip logic, structured
  `uploadTxHash` parsing, `verify.js`, `capture-fixtures.js` + fixtures README +
  provenance, `fetch-references.sh`, offline wasm-hash claim (6/6), CI
  org-namespace scoping, ledger-close wait, checkpoint-footprint widening
  (12 tests). Offline claims 11 pass / 0 fail / 2 skip. Seed-v2 unit tests: 16
  pass. Fixtures: 259 raw RPC responses, 34 recorded hashes verified.
- **Still outstanding / not done:**
  - `settle.js` close/execute is implemented and runnable but **not** wired into
    CI — maturing and executing proposals is a manual operator step.
  - `scripts/activity/activity.js` and `scripts/ttl/extend-seed-contracts.sh`
    exist and are wired into the live tier behind a `.seed/state.json` guard
    (no-ops on hosted CI, active on a self-hosted soak runner), and have **not**
    been exercised against live testnet in CI.
  - `o2`, `o4` and `o6` have **no votes**: they were created in the earlier run,
    so their voting windows closed before the blocker was fixed. This is
    permanent for those proposal ids — new proposals are needed to get abstain
    votes on the `abstain-heavy` shape in the OZ corpus. Script3's
    `s6-abstain-heavy` does carry 3 abstain votes, so abstain is represented.
  - No `ExtendFootprintTtl` has been run against the seed-v2 contracts yet; the
    TTL snapshot above is a read, not an extension. The extension script exists
    but has not been exercised in CI.
  - No `Against` votes exist anywhere in the corpus; the rule set covers the
    `For` and `Abstain` paths against real chain data.
- **Seed-v2 exit criterion: met.** Re-runs from a clean clone; both governors
  produce all six proposal shapes, delegations and on-chain reads.
