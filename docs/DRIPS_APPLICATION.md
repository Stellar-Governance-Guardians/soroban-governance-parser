# Drips Wave application — `soroban-governance-parser`

Submission-ready application material for the Stellar Wave Program. This is a
preparation packet; the maintainer must still complete Drips app onboarding and
the repository application steps.

## Problem

Soroban governance is opaque to the people it is supposed to serve. A governor
emits `ScVal` payloads and XDR events; proposal calldata is often not stored
on-chain at all (OpenZeppelin keeps it in the `propose` transaction). Delegates
and token holders cannot see, in plain language, *what a proposal will do* or
*how risky it is*, and there is no governor-agnostic way to do so across the
governors actually deployed on Stellar.

## What this repo is

`soroban-governance-parser` decodes raw Soroban governance payloads into
human-readable, risk-classified JSON. It is the parser layer of a three-repo
product (parser → indexer → dashboard) and is **governor-agnostic** and
**fail-closed**: anything it cannot verify is labeled `unverified`, never guessed.

## Differentiator

- **Decode + risk + simulate in one pipeline.** A decoded action feeds published
  contextual risk rules, and the same action can be dry-run through
  `simulateTransaction` to an `estimate: true` `ExecutionImpact`.
- **Governor-agnostic by construction.** Governor-specific behavior sits behind
  the `GovernorAdapter` trait; the Script3 adapter is implemented and the
  OpenZeppelin adapter is a bounded next step.
- **Evidence over claims.** Every verified statement is backed by a committed
  raw RPC capture and a machine-checked claim; offline tests pass with the
  network disabled.
- **Zero mock data in production paths** and a published rule table for risk;
  there is no opaque composite score.

## Evidence (all offline, machine-checked)

| Claim | Evidence |
|---|---|
| Script3 adapter reproduces real chain state for proposals 0–5 | `crates/core/tests/differential.rs` over committed captures; `OFFLINE=1 bash scripts/check-claims.sh` |
| 85 tests pass; clippy pedantic is clean | `cargo test --workspace` and CI |
| WASM package has reproducible bytes and SHA-256 | GitHub pre-release `v0.1.0-alpha.2`; `releases/parser-wasm.lock.json`; `scripts/check-wasm-release.py` |
| Dry-run models captured `simulateTransaction` responses | `crates/core/tests/simulate.rs`; `docs/dry-run.md` |
| Schema v1 is frozen and additive-only | `schemas/CHANGELOG.md`; `scripts/check-schema-additive.py` |
| No secrets in Git history or the worktree | CI gitleaks checks |
| Current main is maintained and CI-backed | Main commit `d3ec37be68201055c4bbe5ed58b79d47bd98f4e0`; latest recorded main CI passed |

## Why this is a good Wave repository

This repository is suitable for short, reviewable contribution sprints:

- The core pipeline is deterministic and has no production dependency on test
  fixtures or mock data.
- The public interface is bounded by a frozen v1 schema, a pinned WASM package,
  and the `GovernorAdapter` trait.
- The repository has a pinned Rust toolchain and CI checks for formatting,
  clippy, tests, WASM packaging, claims, links, and secret leakage, plus a
  nightly audit/fuzz tier.
- The backlog contains isolated Rust, CLI, risk-rule, fuzzing, and
  documentation tasks with explicit acceptance tests.
- Contributors can work offline using committed captures; no funded wallet,
  private key, or live testnet access is required for the initial issue set.

The initial Wave should use this parser repository as the anchor project. The
indexer and dashboard are related repositories, but their next milestones
depend on verified event rows and a deployed indexer and should not be bundled
into the first application unless the program requests a multi-repository
application.

## Honest limitations

- **OpenZeppelin adapter not implemented**; its fixtures are captured but the
  adapter is a proposed contributor task.
- **Testnet only.** The seeded governors are pinned upstream commits deployed on
  public testnet; no mainnet DAO is claimed.
- **Single deployment per governor** and no `Against` votes in the corpus.
- **Dry-run is estimate-only.** `sgp simulate` accepts a fully assembled
  transaction envelope and labels the result; proposal-action assembly and
  rent are not modeled.
- **Vote-weight decay:** no decay term was found in either pinned source; the
  parser models none.

## Proposed initial Wave issue set

The following six drafts are the recommended first set. They are deliberately
bounded so a contributor can complete one issue in a one-week sprint:

| Draft | Work | Suggested complexity | Why it is ready |
|---|---|---|---|
| 04 | CLI JSON/table output modes | Trivial | Local CLI change with stable-output tests |
| 05 | Custom-type pretty-printers | High | Pure parser layer with committed fixture coverage |
| 06 | Fuzz targets and corpus growth | Medium | Offline hardening with bounded CI smoke tests |
| 08 | README translation | Trivial | Documentation-only, no runtime dependency |
| 03 | Approval/caller-identity risk rules | High | Pure rules with positive/negative tests |
| 01 | OpenZeppelin adapter skeleton | High | Adapter-boundary work with fail-closed scope |

The suggested complexity values are planning guidance only. Final point values
and complexity must be selected in the Drips interface after approval.

Draft 02 is intentionally excluded because the CLI simulate command is already
implemented locally and its acceptance criteria are complete. It should not be
offered to a Wave contributor as new work.

## Maintainer operating plan

During an active Wave, the organization maintainer will:

1. Review applications and assign contributors daily.
2. Answer scope and evidence questions in the GitHub issue thread.
3. Require the repository PR template, relevant acceptance tests, and CI before
   merging.
4. Mark completed issues resolved before the Wave deadline when acceptance
   criteria are met.
5. Leave timely reviews and provide clear feedback if work is blocked or needs
   revision.

The repository's `CODEOWNERS`, issue templates, PR checklist, pinned CI, and
security policy provide the operating controls for this process.

## Application facts to enter in Drips

- **Organization:** `Stellar-Governance-Guardians`
- **Repository:** `soroban-governance-parser`
- **Ecosystem/program:** Stellar Wave
- **License:** MIT
- **Primary technologies:** Rust, Soroban XDR, WASM, CLI, JSON Schema, and
  GraphQL contract artifacts
- **Repository purpose:** decode and risk-classify Soroban governance data with
  verifiable provenance and fail-closed behavior
- **Current status:** testnet evidence and the parser layer are operational;
  OpenZeppelin support, proposal-action assembly, and mainnet deployment are
  explicitly not claimed
- **Contributor interface:** GitHub issues and pull requests with acceptance
  criteria and test expectations

Do not describe the project as a deployed mainnet DAO, a production indexer, or
an already-complete dashboard. Those statements would contradict the evidence
and reduce reviewer confidence.

## Roadmap

1. OpenZeppelin adapter (bounded) — `docs/wave-issues/01-…`.
2. Proposal-action assembly for a verified subset — new issue after the CLI
   boundary has real users and captures.
3. More contextual risk rules — `docs/wave-issues/03-…`.
4. Hardening, v0.1.0 tagging, and handoff of the WASM lock and SDL to the
   indexer and dashboard.

## Maintainer checklist before submitting

- [x] Repository description, topics, license, README, security policy, and
      contribution guidance are present.
- [x] CI is green on `main`; the latest recorded parser main workflows passed.
- [x] Six contributor-ready issue drafts are selected and scoped above.
- [x] Draft 02 is excluded because it is already implemented.
- [x] Project limitations, testnet-only status, and unavailable live evidence
      are stated explicitly.
- [ ] Confirm organization onboarding in the Drips Wave app.
- [ ] Apply `soroban-governance-parser` to the Stellar Wave Program.
- [ ] Confirm the current program's issue labels and complexity values in the
      Drips interface after repository approval.
- [ ] Open the selected GitHub issues only after repository approval.
- [ ] Confirm a maintainer is available for daily application and PR review
      during the active Wave week.
