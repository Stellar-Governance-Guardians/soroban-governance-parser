# `sgp simulate`: wire the dry-run parser to the CLI


## Summary

`crates/core/src/simulate.rs` already parses and builds `simulateTransaction`
messages, but nothing invokes them: the dry-run parser is exercised only by
offline tests. Add a `simulate` command to the `sgp` CLI so an operator can
dry-run a proposal and get a labeled estimate.

The parser stays sans-IO. The CLI owns the network and the transaction assembly:
build a `TransactionEnvelope` for the proposal's action, call
`build_simulate_request`, send it via `crates/cli/src/rpc.rs`, and pass the raw
response to `parse_simulate_response`.

## Acceptance Criteria

- [x] `sgp simulate <transaction-xdr> [--rpc URL]` prints an `ExecutionImpact`
      (`estimate: true`, `simulated_at_ledger` always present). The command
      accepts a fully assembled transaction envelope rather than guessing how
      to construct a proposal action.
- [x] `--capture <path>` writes the **raw** RPC response to a file so it can be
      committed as a fixture with provenance.
- [x] Absent response fields are printed as `null`, never omitted or zeroed.
- [x] Network errors and malformed responses exit non-zero with an explicit
      message (fail closed).
- [x] The core offline tests cover the parse path from committed captures; the live
      path is documented as testnet-only.
- [x] README quick start documents the command; `docs/dry-run.md` documents the
      estimate contract and remains the reference for response fields.

## Tech Stack

Rust 1.96, `clap` (already used by the CLI), `reqwest` (already in `rpc.rs`),
`stellar-xdr`. No new network dependency.

## Notes / risks

Rent is **not** modeled (`docs/dry-run.md`); the command must not invent a rent
number. Transaction assembly for a proposal action may be the hard part — scope
it to a single decoded `Calldata` action first and fail closed otherwise.
