# Parser

`crates/core` decodes Soroban governance data without IO. `crates/wasm` exposes
it to JavaScript. `crates/cli` is the operator tool, and network calls live
there (`crates/cli/src/rpc.rs`), not in the core.

## What it decodes

| Input | Output |
|---|---|
| base64 XDR `ScVal` | JSON, total over the stellar-xdr 28 type set |
| contract WASM | `contractspecv0` parsed into functions, structs, unions, enums, errors and events |
| raw event | a `NormalizedEvent` through a governor adapter, or `unverified` |
| proposal call | a `DecodedCall` with named arguments when the spec is verified |
| proposal votes | tally and power reconstructed through pure replicas |
| `simulateTransaction` response | an `ExecutionImpact` estimate with `simulatedAtLedger` |

## What it refuses to do

- Guess an argument name without a contract spec: the result is
  `decoding: "unverified"`, not a plausible label.
- Downgrade an unknown function to `low`: it is `UNVERIFIED` with the reason.
- Coerce a value into JSON: integers at 64 bits and above become decimal
  strings, and non-representable maps are errors.
- Emit a dry-run without a ledger: `simulatedAtLedger` is required, and every
  impact carries `estimate: true`.

## Design rules

- **Adapters own governor knowledge.** The `GovernorAdapter` trait
  (`crates/core/src/adapter.rs`) is where a governor's events, calls and state
  are understood. See the [adapter model](adapter-model.md) and the
  [authoring guide](../adapter-authoring.md).
- **Rules are data.** Risk tiers come from the published table in
  `crates/core/src/risk.rs`; every classification exposes the matched rule.
  See [Risk rules](../generated/risk-rules.md).
- **The core does no IO.** Network access is confined to `crates/cli/src/rpc.rs`.

## Pages

- [Decoding rules](decoding-rules.md)
- [Adapter model](adapter-model.md)
- [Script3 adapter](script3-adapter.md)
- [OpenZeppelin adapter](openzeppelin-adapter.md)
- [Risk rules](../generated/risk-rules.md)
- [CLI reference](../generated/cli.md)
- [WASM usage](wasm-usage.md)
- [Dry run (estimate)](dry-run.md)
