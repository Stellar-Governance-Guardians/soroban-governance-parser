# Hardening

How the parser is hardened, and what is not yet covered.

## Dependency audit

```bash
cargo install cargo-audit --locked
cargo audit
```

Run in CI (nightly or on demand). A vulnerability with a fix should be addressed
in a normal PR; one without a fix and not on a reachable path is recorded under
"Honest limitations" with the advisory id rather than silently ignored.

Last run in this environment: **2026-10-08, `cargo-audit` v0.22.2 → clean**
(1,294 advisories loaded, 263 crate dependencies scanned, exit 0).

## Secret scan

```bash
gitleaks detect --source . --no-banner   # full history
```

Requires **gitleaks >= 8.21** (`.gitleaks.toml` uses `[[allowlists]]`, which
older builds ignore — then the known fixture false positives reappear). The two
allowlisted shapes are verified false positives: a JS object property named `key`
with a slug value, and the public base64 `LedgerKey` of a `stateChanges` entry in
verbatim RPC captures. Run before every push (charter rule 6).

## Fuzzing

`fuzz/` holds two `cargo-fuzz` targets for the two hostile-input surfaces:

| target | surface |
|---|---|
| `scval_from_xdr` | the ScVal decoder (`scval.rs`) on arbitrary bytes |
| `parse_spec` | the `contractspecv0` custom-section parser (`spec.rs`/`wasm.rs`) |

`cargo-fuzz` needs **nightly** Rust while the repo pins stable 1.96, so the fuzz
crate is deliberately **outside** the workspace and the PR gate. It runs on the
nightly/manual workflow only.

```bash
cargo install cargo-fuzz
cargo +nightly fuzz run scval_from_xdr -- -max_total_time=60
cargo +nightly fuzz run parse_spec      -- -max_total_time=60
```

Seed corpora are generated from committed fixtures with
`scripts/fuzz/seed-corpus.sh` (real XDR values and the committed `.wasm` files),
so fuzzing starts from real shapes rather than random bytes.

## Not yet covered

- The fuzz targets are committed but have **not** been run to completion here
  (no nightly toolchain in this environment). Treat the first scheduled run as
  the real smoke test.
- No Miri run and no formal verification.
