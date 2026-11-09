# Parser quickstart

Rust workspace: `crates/core` (sans-IO decode), `crates/wasm` (wasm-bindgen),
`crates/cli` (`sgp`).

## Prerequisites

- Rust **1.96.0** — pinned by `rust-toolchain.toml`, so `rustup` picks it up.
- `wasm32-unknown-unknown` target for the WASM crate:
  `rustup target add wasm32-unknown-unknown`.
- `wasm-pack 0.13.1` for packaging the browser/Node builds.

## Build, test, check

```bash
git clone https://github.com/Stellar-Governance-Guardians/soroban-governance-parser
cd soroban-governance-parser

cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings   # pedantic, unwrap/expect/panic denied
cargo test --workspace                                  # 85 tests
OFFLINE=1 bash scripts/check-claims.sh                  # 21 pass, 0 fail, 2 skipped (network)
python3 scripts/check-deployments.py                    # registry vs committed fixtures
```

All of the above run in the PR gate, offline. The two skipped claims need the
public testnet and run in the nightly `live-checks` workflow.

## Decode something

```bash
# A captured `proposal_created` event topic (symbol ScVal), no network needed
cargo run -p soroban-governance-cli --bin sgp -- decode-scval \
  AAAADwAAABBwcm9wb3NhbF9jcmVhdGVk
```

The full command list, including `ttl`, is on the [CLI reference](../generated/cli.md).

## Package the WASM

```bash
wasm-pack build crates/wasm --target nodejs --out-dir pkg/nodejs --out-name sgp_parser_wasm
wasm-pack build crates/wasm --target web    --out-dir pkg/web    --out-name sgp_parser_wasm
node --test crates/wasm/js-tests/node.test.mjs
node crates/wasm/js-tests/browser.test.mjs
```

The exported surface is listed on [WASM usage](../parser/wasm-usage.md). The
current release is `v0.1.0-alpha.2` with sha256
`65193e43642f55abd083e2283aa5ed3caee5839c40a74c7b3f30398ea3ec0629`; the release
lock file is `releases/parser-wasm.lock.json`.

## What is not wired

- `sgp simulate` accepts a validated transaction envelope and emits an
  estimate; it does not assemble proposal actions or estimate rent. See
  [Dry run](../parser/dry-run.md).
- The OpenZeppelin adapter is not implemented. See
  [OpenZeppelin adapter](../parser/openzeppelin-adapter.md).
