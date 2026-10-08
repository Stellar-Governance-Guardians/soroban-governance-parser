# Draft Wave issue — `sgp simulate`: wire the dry-run parser to the CLI

**Suggested labels:** `area:cli`, `complexity:medium` · **Complexity:** Medium (150 pts)

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

- [ ] `sgp simulate --contract <C…> --proposal <id> [--rpc URL]` prints an
      `ExecutionImpact` (`estimate: true`, `simulated_at_ledger` always present).
- [ ] `--capture <path>` writes the **raw** RPC response to a file so it can be
      committed as a fixture with provenance.
- [ ] Absent response fields are printed as `null`, never omitted or zeroed.
- [ ] Network errors and malformed responses exit non-zero with an explicit
      message (fail closed).
- [ ] An offline test covers the parse path from a committed capture; the live
      path is documented as testnet-only.
- [ ] README quick start and `docs/dry-run.md` document the command.

## Tech Stack

Rust 1.96, `clap` (already used by the CLI), `reqwest` (already in `rpc.rs`),
`stellar-xdr`. No new network dependency.

## Notes / risks

Rent is **not** modeled (`docs/dry-run.md`); the command must not invent a rent
number. Transaction assembly for a proposal action may be the hard part — scope
it to a single decoded `Calldata` action first and fail closed otherwise.
