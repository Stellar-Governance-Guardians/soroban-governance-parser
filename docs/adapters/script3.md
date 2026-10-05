# Adapter notes: Script3 soroban-governor

Status: **researched from source, adapter NOT yet implemented** (phase 2 deliverable).
Everything below was read from the project's repository/docs on **2026-10-05**.
Repo lives at `github.com/script3/soroban-governor` (org `script3`, NOT `script3-io`).

## Version verified
- Latest release **v1.1.1** (2024-07-09), built with Soroban CLI 21.0.0; workspace pins
  `soroban-sdk = "20.5.0"`.
  Source: https://api.github.com/repos/script3/soroban-governor/releases ,
  https://github.com/script3/soroban-governor/blob/main/Cargo.toml

## Architecture verified
- Two contracts: **Governor** (`contracts/governor`, crate `soroban-governor`) and
  **Votes/Voter** (`contracts/votes`, crate `soroban-votes`). No separate timelock
  contract — timelock is a Governor setting. Treasury = the Governor contract itself.
- Voter has feature-flag variants: `bonding` (default, wraps a Stellar Asset 1:1),
  admin variant (mint/clawback), `sep-0041` variant.
  Source: https://github.com/script3/soroban-governor/blob/main/docs/architecture.md

## Governor functions verified (contracts/governor/src/contract.rs)
- `initialize(votes: Address, council: Address, settings: GovernorSettings)`
- `settings() -> GovernorSettings`, `council() -> Address`, `vote_token() -> Address`
- `propose(creator: Address, title: String, description: String, action: ProposalAction) -> u32`
- `get_proposal(proposal_id: u32) -> Option<Proposal>`
- `close(proposal_id: u32)` — computes `eta`; **there is NO `queue` function**
- `execute(proposal_id: u32)`, `cancel(from: Address, proposal_id: u32)`
- `vote(voter: Address, proposal_id: u32, support: u32)` (0=Against, 1=For, 2=Abstain)
- `get_vote(voter, proposal_id) -> Option<u32>`, `get_proposal_votes(proposal_id) -> Option<VoteCount>`

## Types verified (contracts/governor/src/types.rs)
- `Proposal { id: u32, config: ProposalConfig, data: ProposalData }`
- `ProposalConfig { title: String, description: String, action: ProposalAction }`
- `ProposalData { creator: Address, vote_start: u32, vote_end: u32, eta: u32, status: ProposalStatus, executable: bool }`
- `ProposalStatus`: Open=0, Successful=1, Defeated=2, Expired=3, Executed=4, Canceled=5
- `VoteCount { against: i128, _for: i128, abstain: i128 }`
- `GovernorSettings { proposal_threshold: i128, vote_delay: u32, vote_period: u32, timelock: u32, grace_period: u32, quorum: u32 (BPS), counting_type: u32 (bitmask), vote_threshold: u32 (BPS) }`
- Proposal calldata: `ProposalAction` enum = `Calldata(Calldata) | Upgrade(BytesN<32>) | Settings(GovernorSettings) | Council(Address) | Snapshot`;
  `Calldata { contract_id: Address, function: Symbol, args: Vec<Val>, auths: Vec<Calldata> }` (recursive; nested auths become `InvokerContractAuthEntry` sub-invocations at execute time).

## Events verified (contracts/governor/src/events.rs)
| topic[0] | topics[1..] | data |
|---|---|---|
| `proposal_created` | proposal_id: u32, proposer: Address | title, description, action, vote_start, vote_end |
| `proposal_canceled` | proposal_id: u32 | () |
| `proposal_voting_closed` | proposal_id: u32, status: u32, eta: u32 | VoteCount |
| `proposal_executed` | proposal_id: u32 | () |
| `proposal_expired` | proposal_id: u32 | () |
| `vote_cast` | proposal_id: u32, voter: Address | support: u32, amount: i128 |

## Votes contract events verified (contracts/votes/src/events.rs)
| topic[0] | topics[1..] | data |
|---|---|---|
| `delegate` | delegator, delegatee | old_delegatee |
| `votes_changed` | delegate | old_votes: i128, new_votes: i128 |
| `set_admin` | admin | new_admin |
| `deposit`/`withdraw`/`claim` (bonding only) | account | amount: i128 |
| `set_emissions` | eps: u64, expiration: u64 | () |

Votes checkpoints: packed `u128` = `(sequence: u32) << 96 | amount (u96)`; per-user current
checkpoint in persistent storage, history `Vec<u128>` in temporary storage; `upper_lookup`
binary search. Functions: `get_votes`, `get_past_votes(user, sequence)`, `get_delegate`,
`delegate(account, delegatee)` (no chaining), `total_supply`, `get_past_total_supply`.
Source: https://github.com/script3/soroban-governor/tree/main/contracts/votes/src

## Published deployments (MAINNET only; from Script3's governance explorer config)
| DAO | Governor | Votes |
|---|---|---|
| YieldBlox | `CANSYFVMIP7JVYEZQ463Y2I2VLEVNLDJJ4QNZTDBGLOOGKURPTW4A6FQ` | `CAZ2U7Z2LS3N72NKMOR34HOV4WFDCMA2TPWD3KU4EA2VKZARFPMLF3LI` |
| Stellar Community Fund | `CD5BO2F56Q7WK37SWASI7J5J46H37ZWEERJWZFJLPQLP6Y6I76MQZCZO` | `CDHUZKXBNMAHTK62HDS6AVIZANVOABKRGY3SP3EUKYE634RFQ5NPG` |
| Soroban Domains | `CDXTT5PBPZ3ERZCC5S6NE2M3NMUNDKL4SJLT26DD3CYUWUWSUV5QPZRW` | `CC4Y5WAI5MF3HF77BAM4YOS3Y4NFZAW3FTRFS25GRLCRBGPDWPYN6VZ3` |

Source: JS config of https://mainnet.governance.script3.io (linked from https://script3.io).

## UNVERIFIED / honest limitations
- **No official testnet deployment IDs found.** The repo Makefile ID is a localhost
  Standalone placeholder. For testnet proof we must deploy via `scripts/seed-testnet.*`.
- Event shapes above are from source reading; not yet observed on live testnet by us.
  Adapter implementation must re-verify against emitted events from our seeded testnet
  deployment before marking anything `verified` at runtime.
- Whether these exact event shapes are emitted by mainnet deployments at v1.1.1:
  UNVERIFIED (we have not queried mainnet).
