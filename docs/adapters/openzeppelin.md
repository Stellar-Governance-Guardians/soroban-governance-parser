# Adapter notes: OpenZeppelin stellar-contracts Governor

Status: **adapter implemented** (`crates/core/src/adapters/openzeppelin.rs`) against the
pinned source and the deployed governor's embedded spec. Proposal state is
capture-verified; event payload layouts are source-verified because no
`getEvents` capture exists for this governor yet. The research below was read
from the repository/docs on **2026-10-05** and re-checked at the deployed pin
`b40c5eaefe6a29f0030f00bd2d730b7a91cce330`.
Repo: `github.com/OpenZeppelin/stellar-contracts` (created 2024-12-11 under this name;
no former name found — claims of a rename are UNVERIFIED/not applicable).

## Version verified
- Latest stable **v0.7.2** (2026-06-09); latest overall **v0.8.0-rc.3** (2026-06-16),
  whose release notes state it is "not yet been audited, and not ready for use in production".
- Governance (Governor/Votes/Timelock) first shipped in **v0.7.0** (2026-04-03).
- Workspace pins `soroban-sdk = "28.0.0"`.
- README describes the project as preliminary/"as-is" software.
  Sources: https://api.github.com/repos/OpenZeppelin/stellar-contracts/releases ,
  https://raw.githubusercontent.com/OpenZeppelin/stellar-contracts/main/Cargo.toml ,
  https://raw.githubusercontent.com/OpenZeppelin/stellar-contracts/main/README.md

## Governor module verified (packages/governance/src/governor/mod.rs, storage.rs)
- `voting_delay()` / `voting_period()` are **in ledgers** (`DAY_IN_LEDGERS = 17280`).
- `counting_mode() -> Symbol` returns `"simple"`.
- `proposal_threshold() -> u128`, `quorum(ledger: u32) -> u128` (checkpoint-based),
  `get_token_contract() -> Address`, `has_voted(proposal_id: BytesN<32>, account) -> bool`.
- `propose(targets: Vec<Address>, functions: Vec<Symbol>, args: Vec<Vec<Val>>, description: String, proposer: Address) -> BytesN<32>`
- `cast_vote(proposal_id: BytesN<32>, vote_type: u32, reason: String, voter: Address) -> u128`
  (0=Against, 1=For, 2=Abstain)
- `queue(targets, functions, args, description_hash, eta: u32, operator) -> BytesN<32>`;
  `execute(...)` / `cancel(...)` have **no default impl** (implementation required);
  `proposals_need_queuing() -> bool` defaults to `false`.
- `get_proposal_id(...)` = keccak256 of XDR-serialized targets/functions/args/description_hash.

## State machine verified
- `ProposalState`: Pending=0, Active=1, Defeated=2, Canceled=3, Succeeded=4, Queued=5,
  Expired=6, Executed=7. Pending/Active/Defeated are time-derived; others stored.
- **CRITICAL FACT: proposal calldata is NOT stored on-chain.** Only
  `ProposalCore { proposer: Address, vote_snapshot: u32, vote_end: u32, state: ProposalState }`
  is stored. Callers re-supply targets/functions/args at queue/execute time; the keccak256
  proposal_id binds them. Consequence for our product: for OZ governors, the indexer must
  capture calldata from `proposal_created` event data (which does carry it) — the contract
  state alone can never show it.

## Events verified (governor/mod.rs `#[contractevent]` structs)
| topic[0] | topics[1..] | data |
|---|---|---|
| `proposal_created` | proposal_id: BytesN<32>, proposer: Address | targets, functions, args, vote_snapshot: u32, vote_end: u32, description: String |
| `vote_cast` | voter: Address, proposal_id | vote_type: u32, weight: u128, reason: String |
| `proposal_queued` | proposal_id | eta: u32 |
| `proposal_executed` | proposal_id | — |
| `proposal_cancelled` | proposal_id | — |
| `quorum_changed` | — | old_quorum: u128, new_quorum: u128 |

Note topic ORDER differs from Script3 for `vote_cast` (voter before proposal_id).

## Votes module verified (packages/governance/src/votes/)
- `get_votes(account) -> u128`, `get_votes_at_checkpoint(account, ledger: u32) -> u128`
  (error FutureLookup=4100 if ledger >= current), `get_total_supply()`,
  `get_total_supply_at_checkpoint(ledger)`, `get_delegate(account) -> Option<Address>`,
  `delegate(account, delegatee)`. Only delegated power counts; no undelegate.
- Checkpoints: `Checkpoint { ledger: u32, votes: u128 }`; keys `Delegatee(Address)`,
  `NumCheckpoints(Address)`, `DelegateCheckpoint(Address,u32)`, `VotingUnits(Address)`.
- Events: `delegate_changed` topics `[.., delegator]` data `[from: Option<Address>, to: Address]`;
  `delegate_votes_changed` topics `[.., delegate]` data `[previous: u128, new: u128]`.

## Timelock verified (packages/governance/src/timelock/)
- Governor queueing: with `proposals_need_queuing()=true`, `queue` moves Succeeded→Queued,
  emits `proposal_queued`. **`eta` is informational — not enforced by the governor itself.**
- `Timelock` trait: `get_min_delay() -> u32` (ledgers), `hash_operation(target, function, args, predecessor, salt)`,
  `schedule/execute/cancel/update_delay` (no default impls; access control left to implementer).
- Events: `operation_scheduled` [id, target] data [function, args, predecessor, salt, delay];
  `operation_executed` [id, target]; `operation_cancelled` [id]; `min_delay_changed`.
- Example integration: `examples/fungible-governor-timelock/governor/src/governor.rs`.

## Deployments
- **UNVERIFIED / none found for governance.** No testnet/mainnet governor contract IDs in
  repo or docs. Only multisig example IDs exist (not governance). For testnet proof we must
  build and deploy the example governor ourselves via `scripts/seed-testnet.*`.

## What the adapter implements

| Piece | Where | Evidence |
|---|---|---|
| Event matching + normalization (`proposal_created`, `vote_cast`, `proposal_queued`, `proposal_executed`, `proposal_cancelled`, `quorum_changed`) | `crates/core/src/adapters/openzeppelin.rs` | pinned source for field layout; deployed embedded spec for event names (`tests/openzeppelin_differential.rs`) |
| `ProposalState` reconstruction from `proposal_state` | `state_from_chain` | six committed `o1`–`o6` captures, differentially tested |
| Calldata projection into `DecodedProposal` | `proposal_from_created` | `proposal_created` map payload; positional args, risk rules applied |
| Registry selection across Script3 + OZ | `adapters::registry` | unit + differential tests |

`#[contractevent]` defaults are load-bearing: topic 0 is the snake_case struct
name, `#[topic]` fields follow in declaration order, and the data payload is a
symbol-keyed map of the remaining fields. `proposal_queued` is absent from the
deployed governor's embedded spec because queuing is disabled and the SDK shakes
unused event specs; the adapter still supports the upstream shape.

## UNVERIFIED / honest limitations
- Whether the v0.7.0 audit covered the governance packages: UNVERIFIED (audit PDF not read).
- Event payload layouts are from pinned source reading; no `getEvents` capture
  for this governor exists yet, so they are not capture-verified.
- The proposal id is format-checked, not recomputed from calldata (the OZ
  keccak256 id rule is implemented in the seed scripts but not in the adapter).
- v0.8.0-rc API may change shapes; adapter targets v0.7.x line.
