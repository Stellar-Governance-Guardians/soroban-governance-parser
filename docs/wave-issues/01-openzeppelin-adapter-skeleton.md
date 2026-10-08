# Draft Wave issue — OpenZeppelin governor adapter (skeleton + fail-closed decode)

**Suggested labels:** `area:adapter`, `complexity:high` · **Complexity:** High (200 pts)

## Summary

The parser has a governor adapter for Script3 but none for OpenZeppelin. Add an
`OpenZeppelinAdapter` behind the existing `GovernorAdapter` trait
(`crates/core/src/adapters/mod.rs`) that identifies OZ governor events and decodes
the on-chain proposal shape, and **fails closed (`unverified`) wherever the
fixtures do not prove a shape**.

Unlike Script3, OpenZeppelin does **not** store action calldata on-chain. The
actions live in the `propose` transaction's host-function arguments. The adapter
must therefore source actions from the committed `propose` captures, recompute
the proposal id with the OZ hashing rule (keccak256 over actions + description
hash; see `scripts/seed-v2/lib/proposals.js`), and compare it with the on-chain
id returned by `proposal_state` — marking the proposal `unverified` on any
mismatch.

## Acceptance Criteria

- [ ] `crates/core/src/adapters/openzeppelin.rs` implements `GovernorAdapter`
      (`name`, `matches_event`, `normalize_event`, `decode_call`) plus
      `state_from_chain` for the captured `proposal_state` reads.
- [ ] Action sourcing uses the committed `propose` captures under
      `tests/fixtures/seed-v2/rpc/` (no network); the recomputed id is compared
      to the on-chain id and a mismatch yields `unverified`, never a guess.
- [ ] An offline differential test (`crates/core/tests/`) covers every captured
      OZ proposal shape `o1`–`o6`; where a shape is absent it is skipped with an
      explicit note, not passed vacuously.
- [ ] `docs/adapters/openzeppelin.md` gets a source-to-behavior mapping table
      like the Script3 one.
- [ ] `OFFLINE=1 bash scripts/check-claims.sh` passes; new claims added to
      `claims.json` for anything the README states.
- [ ] The `oz-adapter-not-implemented` claim is removed/updated when this lands.

## Tech Stack

Rust 1.96 (pinned), `stellar-xdr` 28.0.1, `serde`. Offline tests read committed
fixtures only. Reference source: OpenZeppelin `stellar-contracts` @ the SHA in
`scripts/upstream/upstream.lock.json`.

## Notes / risks

- `o2`, `o4`, `o6` carry no votes in the corpus; the adapter must still decode
  their action shape from the `propose` capture.
- Do not claim the OZ id rule is verified until a recomputed id matches a
  captured on-chain id.
