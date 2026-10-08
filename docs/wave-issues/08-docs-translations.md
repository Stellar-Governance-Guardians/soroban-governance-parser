# Draft Wave issue — translate the README introduction (good first issue)

**Suggested labels:** `area:docs`, `complexity:trivial`, `good-first-issue` · **Complexity:** Trivial (100 pts)

## Summary

The project is Stellar-ecosystem tooling; contributors and reviewers come from
many languages. Add a translated introduction so the project is approachable
without fluent English, starting with one language and a clear pattern for more.

## Acceptance Criteria

- [ ] Add `README.<lang>.md` containing, at minimum, faithfully translated:
      the project description, "What it does", "Honest limitations" and the
      quick start.
- [ ] The English README links to each translation near the top.
- [ ] A `docs/translations.md` describes how to add a language and the rule that
      translations must not introduce claims the English README does not make
      (the claims ledger is authoritative).
- [ ] No translated file claims anything not in `claims.json` (charter rule 8).

## Tech Stack

Markdown only. No code changes; CI already scans tracked Markdown for org
namespace and personal-account rules.

## Notes / risks

Translations are documentation, not contract — keep them clearly marked and
redirect any claim disputes to the English source and `claims.json`.
