# Changelog

Release history for the parser, which carries the suite's tagged releases.
Indexer and dashboard changes are tracked in their own repositories and
summarised on their pages here.

## v0.1.0-alpha.1 — 2026-10-08

Pre-release: parser WASM package.

- Deterministic `wasm-pack` build for the `nodejs` and `web` targets, packaged
  into one `.tgz`.
- Committed release lock with the asset sha256
  `7b2eb9420f0cf0582d079f9603beb1e8548e1c7bac165724688cbae5b9237ecd`
  (245,210 bytes), which is what consumers pin.
- Node test-runner and headless-Chromium tests drive the built artifact against
  committed captures.
- CI gains the `wasm-package` job.
- Exports: `decode_scval_base64`, `parse_contract_spec`, `classify_risk`,
  `script3_decode_proposal`, `script3_state_from_chain`,
  `script3_tally_from_chain`, `evaluate_risk_context`, `evaluate_tally`.

Release notes:
<https://github.com/Stellar-Governance-Guardians/soroban-governance-parser/releases/tag/v0.1.0-alpha.1>

## Schema v1 — 2026-10-08

- `schemas/governance-v1.graphql` frozen with an additive-only gate: the PR gate
  rejects any removal or incompatible change relative to
  `governance-v1.baseline.graphql`.
- sha256 `cfbd85a82ee834ac14723cd5dd73ce18a5aba0313a91a0aaf6053d1a486ae275`,
  pinned by the indexer and the dashboard.

## Earlier milestones

| Date | Change |
|---|---|
| 2026-10-08 | Script3 adapter and snapshot power; sans-IO dry-run model from real captures; hardening (fuzz targets, audit, adapter guide); seed-v2 verification that works from a clean clone |
| 2026-10-06 | Step 0 governance files and charter CI across the three repositories |
| 2026-10-05 | Phase 1 testnet proof: deployments registry and committed fixtures |

## Unreleased

- `v0.1.0-alpha.2`: adapter event surface (`matches_event` / `identify`,
  `normalize_event` with the `unverified` fallback, `decode_call`) and the
  simulation request/response surface.
- Dependency refresh for the open `cargo audit` advisories.
- This documentation site.
