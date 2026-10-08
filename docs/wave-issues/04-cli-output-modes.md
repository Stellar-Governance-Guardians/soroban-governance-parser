# Draft Wave issue — `sgp` output modes (`--json` / table)

**Suggested labels:** `area:cli`, `complexity:trivial`, `good-first-issue` · **Complexity:** Trivial (100 pts)

## Summary

Every `sgp` command prints JSON, which is awkward to scan by hand. Add an opt-in
human-readable table output, keeping JSON as the default so scripts and tests are
unaffected. Good first issue: small blast radius, clear contract.

## Acceptance Criteria

- [ ] A global `--format json|table` flag (default `json`).
- [ ] `table` renders `ttl` and `health` as aligned key/value or columns.
- [ ] Unknown format values exit non-zero with a clear message.
- [ ] Default output is byte-for-byte unchanged (add a test asserting this).
- [ ] README quick start shows one table example.

## Tech Stack

Rust 1.96, `clap`. Add a small formatting module in `crates/cli`; do not add a
table dependency unless it is already in the workspace.

## Notes / risks

Keep machine outputs stable — the claims checker and CI parse `--help`, so don't
rename existing flags.
