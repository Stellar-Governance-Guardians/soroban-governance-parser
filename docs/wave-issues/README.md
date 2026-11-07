# Draft Wave issues

Eight ready-to-open issues for `soroban-governance-parser`. **Open on GitHub only
after the maintainer confirms** (charter: do not apply to Drips or change
visibility without a go-ahead).

| # | title | dependency |
|---|---|---|
| 01 | OpenZeppelin governor adapter (skeleton + fail-closed decode) | none |
| 02 | `sgp simulate`: wire the dry-run parser to the CLI | committed simulate captures |
| 03 | Risk rules for token approvals / caller identity | `docs/adapters/script3.md` rule table |
| 04 | `sgp` output modes (`--json` / table) | none |
| 05 | Soroban custom-type pretty-printers | `parse_contract_spec` |
| 06 | Fuzz targets and a corpus grown from committed fixtures | `fuzz/` |
| 07 | Rent-estimate research for dry-run | a capture that contains a rent figure |
| 08 | Translate the README introduction | none |

## Before opening

- The repository must be approved for the Program first (applies to the whole
  org).
- Filenames are numbered for ordering; the issue title is the `#` heading text.
- Every draft carries a Summary, Acceptance Criteria and Tech Stack section of
  our own; the Program does not mandate a body format.
