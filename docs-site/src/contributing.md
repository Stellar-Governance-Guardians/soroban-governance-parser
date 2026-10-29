# Contributing

The full guide is in [CONTRIBUTING.md](https://github.com/Stellar-Governance-Guardians/soroban-governance-parser/blob/main/CONTRIBUTING.md).
This page summarises what a pull request must satisfy in each repository.

## Rules that apply everywhere

1. **Land every change through a branch and a PR.** One logical unit per
   commit, conventional commit message, and a PR body that lists evidence for
   each "verified" claim.
2. **The PR gate is offline and deterministic.** Tests pass with the network
   disabled. Live-testnet and external-link checks run in the nightly tier and
   never block a merge.
3. **Fixtures are committed raw responses with provenance.** Production code
   never imports them; CI enforces this in every repository.
4. **Cross-repo coupling is by pin.** A commit SHA plus sha256, or a release
   asset URL plus sha256. No relative-path dependency between repositories.
5. **No secrets in git.** gitleaks runs over the worktree and the full history.
6. **Claims are machine-checked.** If a README states a fact, `claims.json` has
   a command that verifies it.
7. **Never loosen a check to make it pass.** Fix the cause, or report the
   blocker.

## Per repository

| Repository | Local gate |
|---|---|
| parser | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `OFFLINE=1 bash scripts/check-claims.sh`, `python3 scripts/check-deployments.py` |
| indexer | `npm run check-pins`, `npm run typecheck`, `npm run lint`, `npm run test:unit`, `npm run test:integration`, `npm run build` |
| dashboard | `npm run check:pins`, `npm run typecheck`, `npm run lint`, `npm run test:unit`, `npm run build`, `npx playwright test`, `bash scripts/check-claims.sh` |

## Documentation

A feature PR is not done until its documentation exists in that repository's
`docs/` and is pinned into this site by a parser-repo PR that bumps
`docs-site/sources.lock`. Generated pages are never edited by hand:

```bash
cd docs-site
python3 scripts/gen-risk-rules.py
python3 scripts/gen-cli.py
python3 scripts/gen-api.py
python3 scripts/gen-evidence.py
python3 scripts/gen-upstream.py
```

Then build and check:

```bash
mdbook build
python3 scripts/check-links.py
python3 scripts/check-docs-claims.py
```

## Code of conduct

Participation is covered by the
[Code of Conduct](https://github.com/Stellar-Governance-Guardians/soroban-governance-parser/blob/main/CODE_OF_CONDUCT.md).
Security reports go through
[SECURITY.md](https://github.com/Stellar-Governance-Guardians/soroban-governance-parser/blob/main/SECURITY.md),
never a public issue.
