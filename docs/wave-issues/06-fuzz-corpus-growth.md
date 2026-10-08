# Draft Wave issue — fuzz targets and a corpus grown from committed fixtures

**Suggested labels:** `area:hardening`, `complexity:medium`, `good-first-issue` · **Complexity:** Medium (150 pts)

## Summary

The ScVal decoder and the `contractspecv0` parser are the two places a hostile
input is most likely to hurt. Add `cargo-fuzz` targets for both and seed their
corpora from the **already-committed** fixtures, so fuzzing starts from real
shapes instead of random bytes.

## Acceptance Criteria

- [ ] `fuzz/` workspace with two targets: `scval_from_xdr` (decode arbitrary
      bytes as a base64/XDR `ScVal`) and `parse_spec` (parse arbitrary bytes as
      WASM custom sections).
- [ ] Seed corpora committed from real fixture bytes
      (`tests/fixtures/seed-v2/rpc/*.json` decoded values and the committed
      `.wasm` files) with a small script that regenerates them.
- [ ] Both targets run without panics for a bounded time; a short smoke run is
      wired into CI (on a schedule or manual trigger, not the PR gate).
- [ ] Any crash found is filed as a separate issue with the reproducer.
- [ ] `docs/hardening.md` documents how to run the fuzzers.

## Tech Stack

Rust nightly + `cargo-fuzz` (libFuzzer). Seeds come from committed fixtures; no
network.

## Notes / risks

`cargo-fuzz` needs nightly while the repo pins stable 1.96, so keep the fuzz
targets out of the main workspace and the PR gate.
