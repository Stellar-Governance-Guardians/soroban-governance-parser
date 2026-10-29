# CLI reference

Generated from the `sgp` binary (`crates/cli`) by `docs-site/scripts/gen-cli.py`. Do not edit by hand: CI regenerates this page and fails on any diff. Binary version: `sgp 0.1.0`.

`sgp` is the operator tool. Network IO lives in `crates/cli/src/rpc.rs`; the core crate stays sans-IO.

## Top-level

```text
Soroban governance payload parser (fail-closed)

Usage: sgp <COMMAND>

Commands:
  decode-scval    Decode a base64 XDR ScVal into JSON. Offline, pure
  spec-from-wasm  Extract and parse the contractspecv0 section from a WASM file. Offline
  fetch-spec      Fetch a contract's WASM from a Soroban RPC and parse its contractspecv0
  health          Print RPC health: latest ledger, oldest ledger, retention window
  ttl             Read a contract's instance/code TTL entries (protocol 29) and check remaining ledgers. Fails closed: missing TTL entries are errors
  help            Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `sgp decode-scval`

Decode a base64 XDR ScVal into JSON. Offline, pure

```text
Decode a base64 XDR ScVal into JSON. Offline, pure

Usage: sgp decode-scval <XDR_BASE64>

Arguments:
  <XDR_BASE64>  base64 XDR of an ScVal (or "-" to read stdin)

Options:
  -h, --help  Print help
```

## `sgp spec-from-wasm`

Extract and parse the contractspecv0 section from a WASM file. Offline

```text
Extract and parse the contractspecv0 section from a WASM file. Offline

Usage: sgp spec-from-wasm <WASM>

Arguments:
  <WASM>  path to contract .wasm

Options:
  -h, --help  Print help
```

## `sgp fetch-spec`

Fetch a contract's WASM from a Soroban RPC and parse its contractspecv0

```text
Fetch a contract's WASM from a Soroban RPC and parse its contractspecv0

Usage: sgp fetch-spec [OPTIONS] --contract <CONTRACT>

Options:
      --contract <CONTRACT>  contract id (strkey C...)
      --rpc <RPC>            Soroban RPC URL (default: SDF public testnet) [default: https://soroban-testnet.stellar.org]
  -h, --help                 Print help
```

## `sgp health`

Print RPC health: latest ledger, oldest ledger, retention window

```text
Print RPC health: latest ledger, oldest ledger, retention window

Usage: sgp health [OPTIONS]

Options:
      --rpc <RPC>  [default: https://soroban-testnet.stellar.org]
  -h, --help       Print help
```

## `sgp ttl`

Read a contract's instance/code TTL entries (protocol 29) and check remaining ledgers. Fails closed: missing TTL entries are errors

```text
Read a contract's instance/code TTL entries (protocol 29) and check remaining ledgers. Fails closed: missing TTL entries are errors

Usage: sgp ttl [OPTIONS] --contract <CONTRACT>

Options:
      --contract <CONTRACT>            contract id (strkey C...)
      --rpc <RPC>                      [default: https://soroban-testnet.stellar.org]
      --min-remaining <MIN_REMAINING>  exit 1 if remaining ledgers fall below this threshold
  -h, --help                           Print help
```
