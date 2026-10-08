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
5. `scripts/seed-v2/capture-fixtures.js` copies the curated subset of
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
node capture-fixtures.js                     # refresh rpc/ + index.json
node verify.js                               # offline integrity of recorded hashes
```

## Honest limitations

- These fixtures record a **partial** seed run. As of 2026-10-08 the run stopped
  at the Script3 S2 (contract-upgrade) propose, which failed on-chain with
  `resource_limit_exceeded`; see `docs/seed-v2.md`. The remaining proposals and
  the OpenZeppelin run are not yet captured.
- `verify.js` currently reports the two historical `uploadTxHash` defects
  (`script3Votes`, `ozMockSubcall` recorded the wasm sha256 instead of a tx
  hash). Those are from the pre-fix run stored in the gitignored `.seed/state.json`;
  the deploy.js fix prevents recurrence and a clean re-run clears them.
- Testnet is periodically reset. If a capture's referenced ledger has aged out of
  retention it stays valid as a fixture, but it can no longer be re-queried live.
