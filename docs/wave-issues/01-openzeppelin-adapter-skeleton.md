# Draft Wave issue — OpenZeppelin event capture + proposal-id recomputation

## Summary

The OpenZeppelin adapter now ships in `crates/core/src/adapters/openzeppelin.rs`:
it matches the pinned governor's events, normalizes their map payloads,
reconstructs `ProposalState` from committed `proposal_state` reads, and projects
`proposal_created` calldata into risk-classified calls. Two proofs are still
missing and are the whole of this issue:

1. **No `getEvents` capture** for the OpenZeppelin governor exists, so event
   payload layouts are transcribed from the pinned source rather than observed
   on chain.
2. The proposal id is only **format-checked** (32 bytes / hex). The upstream OZ
   id is `keccak256(targets, functions, args, description_hash)`; the adapter
   does not recompute it, so it cannot detect a forged or corrupted id.

## Acceptance Criteria

- [ ] Add at least one committed `getEvents` response for
      `CAONSV2R2YCNJS6DSN366M5LXLHWT46ES57MD6KOM725YFQSGRLHDCMM` under
      `tests/fixtures/seed-v2/rpc/`, recorded in `index.json` with its method,
      note, capture time and ledger (raw response only; no hand-edited data).
- [ ] Extend `crates/core/tests/openzeppelin_differential.rs` to normalize each
      captured event and assert its decoded fields against values read
      independently from the raw XDR.
- [ ] Implement the upstream id rule in the adapter (behind a pure function),
      compare it with the `proposal_id` event topic for a committed propose
      capture, and fail closed on mismatch.
- [ ] Keep the existing `proposal_state` differential tests green.
- [ ] Update `docs/adapters/openzeppelin.md`, `README.md`, `SPEC.md` and
      `claims.json` so the capture-verified claim is machine-checked.
- [ ] `OFFLINE=1 bash scripts/check-claims.sh` passes.

## Tech Stack

Rust 1.96 (pinned), `stellar-xdr` 28.0.1, `serde`. Offline tests read committed
fixtures only. Reference source: OpenZeppelin `stellar-contracts` @ the SHA in
`scripts/upstream/upstream.lock.json`.

## Notes / risks

- Producing the `getEvents` capture requires a funded testnet deployer and the
  seed-v2 scripts; the PR must include the raw capture, not a synthetic event.
- `o2`, `o4`, `o6` carry no votes in the corpus. That does not block this issue;
  capture any proposal's creation event.
- Do not mark event layouts capture-verified until the fixture and the
  differential assertions land in the same PR.
