# Draft Drips Wave issues

Seven ready-to-open issues for `soroban-governance-parser`. **Open on GitHub only
after the maintainer confirms** (charter: do not apply to Drips or change
visibility without a go-ahead).

| # | title | area | complexity | points |
|---|---|---|---|---|
| 01 | OpenZeppelin governor adapter (skeleton + fail-closed decode) | adapter | high | 200 |
| 02 | `sgp simulate`: wire the dry-run parser to the CLI | cli | medium | 150 |
| 03 | Risk rules for token approvals / caller identity | risk | medium | 150 |
| 04 | `sgp` output modes (`--json` / table) | cli | trivial | 100 |
| 05 | Soroban custom-type pretty-printers | decode | medium | 150 |
| 06 | Fuzz targets and a corpus grown from committed fixtures | hardening | medium | 150 |
| 07 | Rent-estimate research for dry-run | research | high | 200 |
| 08 | Translate the README introduction | docs | trivial | 100 |

Good first issues: 04, 06, 08.

## Format — what is confirmed from the Drips docs

Read from the Wave maintainer pages (`docs.drips.network/wave`,
`…/wave/maintainers/participating-in-a-wave/`) on 2026-10-08:

- Complexity determines Points: **Trivial = 100**, **Medium = 150**, **High = 200**.
- Issues are added to a Program either from the Maintainers → Issues dashboard
  **or** by applying the Program's label (the bot also posts a comment and applies
  a label, e.g. `Stellar Wave`) — the latter only works if the repo is already
  approved for that Program.
- Reviews and points are handled by the Wave app; issues not resolved roll over
  to the next cycle.

## What I could NOT confirm (verify before opening)

- The **exact label strings** the current Stellar Wave Program uses
  (the docs show `Stellar Wave` as an example; `area:*` / `complexity:*`
  labels below are a proposal, not a confirmed schema).
- Whether `good-first-issue` is still the recommended convention for Wave.
- The Program's current issue-body template, if any — the docs describe
  complexity/points but do not mandate a body format, so the drafted
  Summary / Acceptance Criteria / Tech Stack structure is our own.

## Before opening

- Repo must be approved for the Program first (applies to the whole org).
- Filenames are numbered for ordering; issue titles are the `#` heading text.
- Each issue should be labelled with the Program label plus the area/complexity
  labels once confirmed.
