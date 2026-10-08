# tests/fixtures/seed-v2 — provenance ledger (seed v2)

Charter rules 1 and 3: every file here is a **verbatim raw Soroban RPC response**
captured from live Stellar testnet, so the offline differential tests keep
working after the public RPC retention window (120,960 ledgers) expires. Nothing
here is hand-written mock data, and nothing is edited between the wire and the
file.

## What is here

| path | what |
|---|---|
| `wasms/*.wasm` | the six pinned upstream governor artifacts (Script3 + OpenZeppelin) built byte-reproducibly; hashes recorded in `scripts/upstream/upstream.lock.json` |
| `rpc/*.json` | curated raw RPC responses (getHealth / getTransaction / getEvents / getLedgerEntries / simulateTransaction) |
| `index.json` | provenance manifest for every committed RPC response (method, note, capture time, and derived txHash/status/ledger where present) |

## How they were produced

1. `scripts/upstream/fetch-references.sh` clones the upstream repos into
   `references/` at the pinned SHAs; `scripts/upstream/build-upstream.sh` builds
   and verifies the wasm hashes against `upstream.lock.json`.
2. `scripts/seed-v2/deploy.js` deploys the six upstream contracts on live
   testnet (deploy txs captured raw).
3. `scripts/seed-v2/seed.js` initializes both governors, mints voting power,
   casts real delegations, creates proposals, and casts real votes.
4. `scripts/seed-v2/settle.js` closes/executes proposals once their vote windows
   mature.
5. `scripts/seed-v2/snapshot-power.js` captures power-at-snapshot reads
   (`get_past_votes(voter, vote_start)` and `get_past_total_supply(vote_start)`)
   for every Script3 proposal — the weights the governor credits to each cast
   vote. These are read-only simulations (never submitted).
6. `scripts/seed-v2/capture-fixtures.js` copies the curated subset of
   `.seed/capture/` (gitignored) into `rpc/` and writes `index.json`.

## Sanitization

`.seed/` (keys, signed envelopes, the full raw log) is gitignored and never
committed. `capture-fixtures.js` copies only the curated, non-secret RPC
responses above. Secret scanning (`gitleaks`, see `.gitleaks.toml`) runs before
every push; the two allowlist entries there are verified false positives
(a slug-valued object property named `key`, and the public base64 XDR
`LedgerKey` of a `stateChanges` entry).

## Reproduce

```bash
scripts/upstream/fetch-references.sh
scripts/upstream/build-upstream.sh          # verifies pinned wasm hashes
cd scripts/seed-v2 && npm ci
node deploy.js && node seed.js && node settle.js
node snapshot-power.js                     # power-at-snapshot reads (N3)
node capture-fixtures.js                     # refresh rpc/ + index.json
node verify.js                               # offline integrity of recorded hashes
```

## Honest limitations

- **259 committed RPC responses**, ledgers 5083606–5086043 (the later
  `get_past_votes`/`get_past_total_supply` snapshot reads were captured
  2026-10-08 by `snapshot-power.js`; the earlier 232 are unchanged).
  `verify.js` checks **34 recorded hashes** against 68 captures; all 34 are
  SUCCESS transactions and none is a wasm sha256. From a **clean clone** (no
  `.seed/state.json`) it falls back to checking all **68 committed**
  `getTransaction` hashes against their captures: 65 SUCCESS plus the 3
  documented FAILED mints below, none a wasm sha256.
- **Three transactions are intentionally `FAILED`** and are kept, not
  overwritten: the OpenZeppelin mints to `delegate-1/3/5` that trapped on
  `[TotalSupplyCheckpoint, 1]`
  (`803d0bc41e1252603c78ae714080f32797b3751032582f19db3ce9ce0651d59e`,
  `2aef69bdbca83ad47341d9c604cd41ca70fb2bfef62a8bcb18037531d6906945`,
  `e352ae22bf264bbc3f80af3f8d9c44431ffdef1938374898777bb6f7a94c2b60`). The same
  mints later appear as SUCCESS in the same directory — that before/after pair is
  the evidence for the fix. See `docs/seed-v2.md`.
- **`o2`, `o4` and `o6` (OpenZeppelin) carry no votes.** They were created in an
  earlier run, so their voting windows closed before the blocker was fixed; vote
  windows cannot be reopened. Abstain is still represented in the corpus by
  Script3's `s6-abstain-heavy` (3 abstains).
- **No `Against` votes exist anywhere in the corpus.** The `For` and `Abstain`
  paths are backed by real chain data; `Against` is not.
- **`settle.js` close/execute has not been run** against these proposals, so no
  `Executed`/`Defeated`/`Succeeded` terminal states are captured beyond whatever
  the governor already reported.
- `verify.js` also checks the two historical `uploadTxHash` defects
  (`script3Votes`, `ozMockSubcall` recorded the wasm sha256 instead of a tx
  hash). Those come from the pre-fix run stored in the gitignored
  `.seed/state.json`; the `deploy.js` fix prevents recurrence.
- Testnet is periodically reset and these contracts have a TTL. If a capture's
  referenced ledger has aged out of retention, or the contract has expired, the
  fixture stays valid but can no longer be re-queried live. That is the whole
  reason the responses are committed.

## Re-seed after a testnet reset

```bash
scripts/upstream/fetch-references.sh
scripts/upstream/build-upstream.sh
cd scripts/seed-v2 && npm ci
node deploy.js && node seed.js && node settle.js
node snapshot-power.js
node capture-fixtures.js && node verify.js
```

If the contracts are merely expired rather than gone, TTL can be extended
without re-deploying:

```bash
bash scripts/ttl/extend-seed-contracts.sh   # reads contract ids from .seed/state.json
```
