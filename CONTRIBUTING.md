# Contributing

Thanks for helping the Stellar-Governance-Guardians suite — a three-repo program
(parser → indexer → dashboard) run under a written charter. This document covers
the mechanics. The charter rules below are enforced by CI, not by convention.

## Branch and merge policy (`main` is protected)

`main` is guarded by a repository ruleset:

- **Pull request required.** Direct pushes and force pushes to `main` are rejected.
- **CI required.** All required checks must pass before merge (list below).
- **Linear history required.** Rebase or squash only; merge commits are rejected.
- **Required approvals: 0 while the project is single-owner.** This is deliberate:
  the sole maintainer cannot approve their own PR, so requiring 1 would deadlock
  the repository. Raise this to 1 in the same change that adds a second
  maintainer with write access, and update this document.

Required checks for this repository:

| check | verifies |
|---|---|
| `fmt / clippy-pedantic / test / wasm` | `cargo fmt`, pedantic clippy `-D warnings`, workspace tests, release wasm build |
| `charter rules (fixtures, namespace, claims)` | no fixture imports in production code, org-namespace URLs only, claims ledger passes |

CI runs on the Rust toolchain pinned in `rust-toolchain.toml`. Never loosen the
pin to make CI pass; fix the code.

## Workflow

1. Branch from the latest `main`: `git switch -c <type>/<short-name> origin/main`.
2. Make one scoped change. The program workflow is
   READ → PLAN → BUILD → PROVE → DOCUMENT → AUDIT → REPORT.
3. Run what you can locally — CI is the authoritative gate.
4. Open a PR using the template. Fill the checklist in honestly.
5. Merge with `gh pr merge <n> --rebase` (or `--squash`) when checks are green.

## Commit style

Conventional commits: `feat:`, `fix:`, `docs:`, `chore:`, `ci:`, `test:`,
`refactor:`. One logical change per commit. Explain *why* in the body when it is
not obvious from the diff.

## Charter rules (standing, enforced)

1. **Zero mock data in production paths.** CI fails if production code imports
   `tests/fixtures`. Fixtures are live captures or outputs of the seed scripts.
2. **Fail closed, fail loud.** Unverifiable decoding returns
   `decoding: "unverified"` with the raw payload; it is never guessed.
3. **Governor-agnostic.** Chain access goes through the `GovernorAdapter` trait.
4. **Estimates are labeled.** Simulated values are estimates; absent fields are
   `null`, never invented.
5. **No secrets.** No keys, tokens, or credentials in any file, branch, or history.
6. **Org namespace only.** Tracked files never reference personal-account URLs.
7. **Claims ledger.** Any machine-checkable claim in docs gets an entry in
   `claims.json` with the exact command that verifies it.
8. **Delegate metrics are data, not verdicts.** Always expose numerator and
   denominator; never a single opaque score.
9. **v1 is read-only.** The dashboard holds no keys and signs nothing.

## Fixtures and evidence

Every fixture carries provenance (network, RPC, ledger, tx hash) in
`tests/fixtures/README.md`. No provenance, no commit. Evidence files print full
hashes — never truncate.

## Security

See [SECURITY.md](SECURITY.md). Do not open public issues for vulnerabilities.
