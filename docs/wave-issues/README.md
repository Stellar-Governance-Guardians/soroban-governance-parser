# Drips Wave issue preparation — parser

Eight drafted issues for `soroban-governance-parser`; six are recommended for
the initial Wave application.

**Open on GitHub only after repository approval.** The repository is not yet
approved for a Wave program, and issues in this organization are untagged:
nothing here prescribes a label, a points value, or a difficulty rating.

| # | title | area |
|---|---|---|
| 01 | OpenZeppelin governor adapter (skeleton + fail-closed decode) | adapter |
| 02 | `sgp simulate`: wire the dry-run parser to the CLI | cli |
| 03 | Risk rules for token approvals / caller identity | risk |
| 04 | `sgp` output modes (`--json` / table) | cli |
| 05 | Soroban custom-type pretty-printers | decode |
| 06 | Fuzz targets and a corpus grown from committed fixtures | hardening |
| 07 | Rent-estimate research for dry-run | research |
| 08 | Translate the README introduction | docs |

## Recommended first set

Open 01, 03, 04, 05, 06, and 08 after approval. They are independently
actionable with committed fixtures or no network dependency and have explicit
acceptance criteria.

Do not open 02 as new work: it is implemented locally and the draft records the
acceptance criteria already met. Hold 07 for a later Wave because it is a
higher-uncertainty research task and should not compete with the first set.

## Before opening

- The organization must be approved for the program first.
- Issue titles are the `#` heading text; the body structure (Summary,
  Acceptance Criteria, Tech Stack, Notes) is ours, not a program template.
- Do not add labels or points. Any scheme the program defines comes from the
  program, not from this file.
