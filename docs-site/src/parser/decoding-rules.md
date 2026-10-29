# Decoding rules

`crates/core/src/scval.rs` converts an `ScVal` to JSON. The mapping is total
over the stellar-xdr 28 type set and preserves value exactly; it does not
approximate.

## Value mapping

| `ScVal` kind | JSON |
|---|---|
| `Void` | `null` |
| `Bool` | `true` / `false` |
| `U32`, `I32` | number |
| `U64`, `I64`, `Timepoint`, `Duration` | `{ "_type": "u64", "value": "<decimal>" }` (and `i64`, `timepoint`, `duration`) |
| `U128`, `I128`, `U256`, `I256` | `{ "_type": "i128", "value": "<decimal>" }` (and `u128`, `u256`, `i256`) |
| `Bytes` | `{ "_type": "bytes", "value": "<base64>" }` |
| `String`, `Symbol` | string |
| `Address` (account) | `{ "_type": "address:account", "value": "G…" }` |
| `Address` (contract) | `{ "_type": "address:contract", "value": "C…" }` |
| `Vec` | array |
| `Map` with string or symbol keys | object |
| `Map` with any other key type | **error**, not a stringified key |
| `Error` | `{ "_type": "error", "value": "contract:5" }` |
| `LedgerKeyContractInstance` | `{ "_type": "ledger_key_contract_instance", "value": null }` |
| `LedgerKeyNonce` | `{ "_type": "ledger_key_nonce", "value": "<nonce>" }` |

Two rules matter in practice:

1. **Wide integers keep their width and value as a string.** A 128-bit token
   amount cannot round-trip through a JavaScript number, so it stays a tagged
   object with a decimal string (`{"_type":"i128","value":"…"}`) all the way
   to the page. Consumers that need arithmetic parse it explicitly.
2. **Non-representable maps are errors.** Stringifying a struct key would
   silently change the value's identity, so the decoder refuses.

## Argument decoding

Positional `ScVal` arguments are zipped with the input names declared by the
contract's own `contractspecv0` section:

- names verified against the spec ⇒ `decoding: "VERIFIED"`, arguments as a named
  object;
- spec missing or governor unrecognized ⇒ `decoding: "UNVERIFIED"`, arguments as
  a positional array;
- arity mismatch ⇒ explicit error.

`governor: "unverified"` is a literal value in the output, never a guess at
which governor produced a call.

## Risk classification

Every decoded call carries a `RiskClassification`:

```json
{
  "tier": "CRITICAL",
  "matchedRule": "transfer",
  "verified": true
}
```

- The tier comes from an exact, case-sensitive match on the verified function
  name; the generated [rule table](../generated/risk-rules.md) is the whole list.
- No match ⇒ `tier: "UNVERIFIED"` with
  `matchedRule: "no published rule matched this function name"`.
- A function name that could not be verified against a spec is always
  `UNVERIFIED`, even if its text matches a rule.
- Contextual rules (`treasury_outflow`, `self_call`, `large_value`,
  `batched_actions`) add evidence from the arguments; they never change the
  function-name tier and never collapse into a composite score.

## Tally and power

Votes are not summed ad hoc. Two pure replicas reproduce the pinned Script3
behaviour:

- `crates/core/src/checkpoint.rs` — checkpointed voting power, so a vote counts
  the power held at the proposal's snapshot ledger, not at vote time;
- `crates/core/src/tally.rs` — tally and quorum arithmetic with the contract's
  rounding.

Both are covered by property tests (zero votes, exact ties, `i128` extremes,
empty checkpoint history) and by differential tests against committed on-chain
reads.
