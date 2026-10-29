# Risk rules

Generated from `crates/core/src/risk.rs` by `docs-site/scripts/gen-risk-rules.py`. Do not edit by hand: CI regenerates this page and fails on any diff.

Classification is an exact, case-sensitive match on the **verified** function name. A function name that matches no rule is `UNVERIFIED` — never downgraded to `LOW`. Every classification exposes the rule that matched, so no tier is an opaque score.

The table holds **31 rules** across 4 tiers.

## Critical — 11 rules

_Rules that can move value or change contract code/ownership._

| Function name | Why it is this tier |
|---|---|
| `transfer` | moves token value |
| `transfer_from` | moves token value on behalf of another account |
| `mint` | creates new token supply |
| `burn` | destroys token supply |
| `upgrade` | replaces contract WASM |
| `update_wasm_hash` | replaces contract WASM |
| `set_admin` | changes contract administrator |
| `change_admin` | changes contract administrator |
| `transfer_ownership` | changes contract owner |
| `renounce_ownership` | removes contract owner |
| `execute` | executes a queued governance payload; effect depends on payload |

## High — 8 rules

_Rules that change governance parameters or state._

| Function name | Why it is this tier |
|---|---|
| `queue` | schedules a passed proposal for execution |
| `cancel` | cancels a proposal |
| `set_voting_delay` | changes governance timing |
| `set_voting_period` | changes governance timing |
| `set_quorum` | changes quorum requirement |
| `set_proposal_threshold` | changes who may propose |
| `update_config` | changes contract configuration |
| `set_config` | changes contract configuration |

## Medium — 5 rules

_Rules with observable but bounded effects._

| Function name | Why it is this tier |
|---|---|
| `propose` | creates a proposal; effect deferred to execution |
| `delegate` | moves voting power between accounts |
| `vote` | casts a vote |
| `cast_vote` | casts a vote |
| `approve` | grants a token allowance |

## Low — 7 rules

_Read-only or informational actions._

| Function name | Why it is this tier |
|---|---|
| `get_proposal` | read-only accessor |
| `get_votes` | read-only accessor |
| `balance` | read-only accessor |
| `decimals` | read-only accessor |
| `name` | read-only accessor |
| `symbol` | read-only accessor |
| `version` | read-only accessor |

## Contextual rules

The rules above match on the function name. These additional rules evaluate the decoded arguments and emit every rule that fired, each with its evidence:

- `batched_actions`
- `large_value`
- `self_call`
- `treasury_outflow`

Contextual rules never change the function-name tier; they add evidence next to it. See `docs-site/src/parser/decoding-rules.md` for what a caller receives.
