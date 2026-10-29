# delegate-portal-dashboard

Read-only delegate and governance dashboard for Soroban DAOs on Stellar: decode
raw governance payloads in your browser with the pinned parser WASM, and read
live indexer health without ever holding a key.

Part of the Stellar Governance Guardians suite:
[parser](https://github.com/Stellar-Governance-Guardians/soroban-governance-parser) →
[indexer](https://github.com/Stellar-Governance-Guardians/governance-event-indexer) →
**dashboard**.

## Status: app scaffold + `/simulate` + `/status` — data pages not built

This repository contains the application scaffold (Next.js 16 App Router,
strict TypeScript, Tailwind, Vitest, Playwright), the pinned interface
artifacts, and two working surfaces:

| Route | What it does | Depends on |
|---|---|---|
| `/simulate` | Decodes a pasted base64 XDR `ScVal` **locally in the browser** with the pinned `soroban-governance-wasm` build. Validates charset, length (64 KiB) and base64 framing; failures are shown verbatim. | parser WASM only |
| `/status` | Shows the exact interface pins this build uses and the indexer's `indexerHealth`, including the machine-readable reason when it is unavailable. | `INDEXER_URL` |
| `/` | Product summary, indexer health, and an explicit list of what is not built. | `INDEXER_URL` |

Proposal, delegate and execution-impact pages are **not** built: they need the
indexer's decoded tables, and the indexer stops at raw ingestion. Its decode
milestone needs an event-normalization export in the parser WASM that does not
exist in `v0.1.0-alpha.1`.

## Run it

```bash
npm ci
npm run dev            # http://localhost:3000
```

`npm run dev`/`build`/`test` first run `scripts/unpack-wasm.mjs`, which verifies
the vendored release tarball against `parser.lock` and unpacks the pinned WASM
into `generated/wasm/` (gitignored). No network access is needed for any of it.

To show indexer data, point the dashboard at a running
[governance-event-indexer](https://github.com/Stellar-Governance-Guardians/governance-event-indexer)
GraphQL endpoint:

```bash
INDEXER_URL=http://127.0.0.1:4000/graphql npm run dev
```

If `INDEXER_URL` is unset or unreachable, every data surface shows an explicit
error with the reason. There is no bundled sample dataset, no cached fallback
and no silent zero.

## Checks

```bash
npm run typecheck     # tsc --noEmit (strict, noUncheckedIndexedAccess)
npm run lint          # eslint (Next core-web-vitals + no-fixture-imports)
npm run test:unit     # Vitest: 25 tests (unit + component)
npm run test:e2e      # Playwright against `next build && next start`
npm run check:pins    # sha256 of the vendored WASM + SDL vs the lock files
bash scripts/check-claims.sh   # machine-checks the claims made in this README
```

The unit and component tests use the **real** pinned WASM (nodejs target) as
their decoder — there is no mock decode path. The e2e run uses system Chrome
and deliberately leaves `INDEXER_URL` unset to prove the fail-loud state. The
GitHub workflow runs all of the above plus the charter checks (community files,
organization-only links, no personal account names, no fixture imports, no
secrets via gitleaks).

## Interface pins (D1)

| Artifact | Pin | Verified by |
|---|---|---|
| GraphQL SDL `governance-v1` | `schema.lock` — parser commit `ba3bf1a3…`, sha256 `cfbd85a8…` | `npm run check:pins` |
| Parser WASM `v0.1.0-alpha.1` | `parser.lock` — GitHub Release asset, sha256 `7b2eb942…` | `npm run check:pins`, `scripts/unpack-wasm.mjs` |

Committed cross-repo references are content pins only. Nothing in `app/`,
`components/` or `lib/` reads a sibling repository by relative path.

## Architecture

```
app/                 routes (server components read INDEXER_URL; /simulate is client-side)
app/api/             (none yet — no server-side data transformation is needed)
components/          presentational, testable, no data fetching
lib/decode.ts        input validation + decode orchestration (pure, tested)
lib/graphql.ts       strict GraphQL client: no coercion, no defaults, no fallback data
lib/pins.ts          reads the lock files for /status
lib/wasm.ts          loads the pinned WASM (browser)
scripts/             pin check, WASM unpack, claims runner
e2e/                 Playwright specs
```

Data rules in force:

- The indexer is the only data source. A missing or broken indexer is an error
  state, never an empty table that reads as "no data".
- Numbers are rendered exactly as the API returned them; nothing is rounded
  away or composited into a score.
- The app is read-only: no wallet, no signing, no keys, no server-side secrets.

## Honest limitations

- Only `decode_scval_base64` from the pinned WASM is used. Event XDR,
  transaction-hash lookup, proposal call decoding and risk classification all
  need parser WASM exports that ship with `v0.1.0-alpha.2` (not published yet).
- Codegen of typed GraphQL operations is deferred until the indexer exposes the
  full schema; today the one query and its response shape are hand-written and
  strictly validated in `lib/graphql.ts`.
- `/status` reads pins from the working tree (`parser.lock`, `schema.lock`) and
  shows them as build dependencies; it cannot prove what a *deployed* instance
  was built from.
- Lighthouse accessibility scores have not been measured and are not claimed.
  The UI has landmarks, a skip link, visible focus, labelled controls, an
  `aria-live` result region and light/dark tokens; formal audit is pending.
- No live-indexer page has been exercised end to end, because the indexer does
  not expose GraphQL yet.

## License

MIT — see [LICENSE](./LICENSE).
