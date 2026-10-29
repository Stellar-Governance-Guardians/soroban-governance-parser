# Adapter model

A governor is understood by exactly one module behind the `GovernorAdapter`
trait (`crates/core/src/adapter.rs`). Nothing outside the adapter needs to know
the governor exists — that is what keeps governor logic out of TypeScript.

## The trait

| Method | Responsibility |
|---|---|
| `name()` | stable adapter id, e.g. `script3-soroban-governor` |
| `matches_event(event)` | is this event ours? contract id + topic symbol + exact arity; `false` on any doubt |
| `normalize_event(event)` | decode topics and data into a `NormalizedEvent`; `Err` on an unexpected shape |
| `decode_call(call, spec)` | decode a proposal action into a `DecodedCall`; the spec supplies argument names |

Governor-specific helpers — state and tally reconstruction, action enums,
contextual rule projection — live in the same module.

## Rules that are not optional

1. **Fail closed.** Unknown variant ⇒ a preserved `Unknown` and an `Err`, never
   a plausible guess. Missing field ⇒ `Err`. Absent spec ⇒
   `decoding: "unverified"`.
2. **Exact matching.** `matches_event` demands the contract id, the topic symbol
   and the exact topic arity. A superset match would decode another governor's
   events under the wrong rules.
3. **No governor logic elsewhere.** Reimplementing proposal state, tally or
   action variants in the indexer or the dashboard duplicates the parser's
   golden tests and drifts from them. The consumers call the pinned WASM.
4. **Unknown is data.** A `ProposalAction` variant the adapter does not know is
   preserved with its raw payload, not dropped and not mapped to a known one.

## Implementation status

| Adapter | Status | Where |
|---|---|---|
| Script3 (`script3-soroban-governor`) | implemented | `crates/core/src/adapters/script3.rs` |
| OpenZeppelin | not implemented; fixtures and research committed | `docs/adapters/openzeppelin.md` |

To add a governor, follow the [authoring guide](../adapter-authoring.md).
