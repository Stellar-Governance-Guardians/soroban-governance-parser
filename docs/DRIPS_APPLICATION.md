# Drips Wave application — soroban-governance-parser

Draft application material. **Do not submit** until the maintainer confirms
(checklist at the bottom).

## Problem

Soroban governance is opaque to the people it is supposed to serve. A governor
emits `ScVal` payloads and XDR events; proposal calldata is often not stored
on-chain at all (OpenZeppelin keeps it in the `propose` transaction). Delegates
and token holders cannot see, in plain language, *what a proposal will do* or
*how risky it is*, and there is no governor-agnostic way to do so across the
governors actually deployed on Stellar.

## What this repo is

`soroban-governance-parser` decodes raw Soroban governance payloads into
human-readable, risk-classified JSON. It is the first layer of a three-repo
product (parser → indexer → dashboard) and is **governor-agnostic** and
**fail-closed**: anything it cannot verify is labeled `unverified`, never guessed.

## Differentiator

- **Decode + risk + simulate in one pipeline.** Most tooling does one of these.
  Here a decoded action feeds published contextual risk rules, and the same
  action can be dry-run (`simulateTransaction`) to an `estimate: true`
  `ExecutionImpact`.
- **Governor-agnostic by construction.** All governor specifics sit behind the
  `GovernorAdapter` trait; the Script3 adapter is implemented and the
  OpenZeppelin adapter is a scoped next step.
- **Evidence over claims.** Every "verified" statement is backed by a committed
  raw RPC capture and a machine-checked claim (`claims.json`); offline tests pass
  with the network disabled.
- **Zero mock data in production paths** (CI-enforced) and a **published rule
  table** for risk — no opaque scores.

## Evidence (all offline, machine-checked)

| claim | evidence |
|---|---|
| Script3 adapter reproduces real chain state for proposals 0–5 | `crates/core/tests/differential.rs` over committed captures; `OFFLINE=1 bash scripts/check-claims.sh` |
| 85 tests pass; clippy pedantic clean | `cargo test --workspace` |
| WASM package published with a reproducible sha256 | GitHub pre-release `v0.1.0-alpha.1`; `releases/parser-wasm.lock.json`; `scripts/check-wasm-release.py` |
| dry-run models real `simulateTransaction` responses | `crates/core/tests/simulate.rs`; `docs/dry-run.md` |
| schemas v1 frozen, additive-only | `schemas/CHANGELOG.md`; `scripts/check-schema-additive.py` |
| no secrets in git history | `gitleaks detect` (full history) |

## Honest limitations

- **OpenZeppelin adapter not implemented** (its fixtures are captured).
- **Testnet only.** The seeded governors are pinned upstream commits deployed on
  public testnet; no mainnet DAO is claimed.
- **Single deployment per governor**; no `Against` votes in the corpus.
- **Dry-run is parser-side only** — no CLI `simulate` command yet; rent not
  modeled.
- **Vote-weight decay**: no decay term found in either pinned source (a source
  reading, not a live-verified claim); the parser models none.

## Roadmap

1. OpenZeppelin adapter (bounded) — `docs/wave-issues/01-…`.
2. CLI `simulate` wiring — `docs/wave-issues/02-…`.
3. More contextual risk rules — `docs/wave-issues/03-…`.
4. Hardening (fuzz + audit), v0.1.0 tag, then hand the WASM lock + SDL to the
   indexer and dashboard.

## Maintainer checklist before submitting

- [ ] Repo description + topics set (needs repo-admin rights; `gh repo edit` was
      refused for the current token with HTTP 403).
- [ ] CI green on `main`.
- [ ] Wave issues opened from `docs/wave-issues/*.md` (5–8), labelled.
- [ ] Confirm the current Wave Program's required label names and complexity
      values (see `docs/wave-issues/README.md` for what is unconfirmed).
- [ ] Confirm the org is onboarded to the Stellar Wave Program and this repo is
      approved before opening issues.
