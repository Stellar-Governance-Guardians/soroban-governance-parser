# Dry-run (`simulateTransaction`) modeling

Sans-IO parser side of the dry-run feature. Code: `crates/core/src/simulate.rs`;
offline tests: `crates/core/tests/simulate.rs` (committed real captures).

## Charter rule 5

Every dry-run output is an **estimate**. `ExecutionImpact.estimate` is always
`true`, and `simulated_at_ledger` is mandatory — a response without
`latestLedger` is an error, never a default. Simulation runs against the current
ledger, not the future execution ledger; timelocks and state changes between now
and execution can change the real result.

## Modeled fields

Modeled against the committed raw captures under
`tests/fixtures/seed-v2/rpc/*-simulateTransaction-*.json`. The RPC
`response.result` object carries exactly:

| RPC field | `ExecutionImpact` field | notes |
|---|---|---|
| `latestLedger` | `simulated_at_ledger` | always present; required |
| `minResourceFee` | `min_resource_fee_stroops` | string; absent ⇒ `null` |
| `stateChanges` | `state_changes` | present only when the call writes; else `null` |
| `results` | (presence only) | presence ⇒ success; XDR not decoded here ⇒ `decode_status: unverified` |
| `cost.cpuInsns` | `cpu_instructions` | **not present in any capture** ⇒ always `null` |
| `cost.memBytes` | `memory_bytes` | **not present in any capture** ⇒ always `null` |

Absent means `null`; nothing is defaulted to zero or to "success".

## Rent

**Not modeled.** The captured responses contain no rent figure, and the Soroban
rent model depends on entry TTLs and ledger timing the simulation response does
not expose directly. Rather than derive a number from an assumption, the field is
omitted and this limitation recorded. A future capture that includes a rent
figure (or a documented derivation from `stateChanges` durability + TTL) can add
it.

## IO split

`build_simulate_request(transaction_xdr_base64, request_id)` returns the JSON-RPC
2.0 request body and validates the envelope (fail-closed). Assembling the
transaction from a decoded `action`, and sending it, is IO and lives in the CLI's
`RpcClient`; the core crate stays sans-IO.

## Not yet done

- The CLI does not expose a `simulate` command yet, so this is exercised by the
  offline tests, not by an operator command.
- `decode_status` is `unverified`: the impact parser does not decode the result
  XDR against a contract spec.
