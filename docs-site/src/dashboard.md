# Dashboard

`delegate-portal-dashboard` is a read-only Next.js application: no wallet, no
signing, no keys, no server-side secrets.

> **Status: scaffold merged. Two working surfaces; proposal, delegate and
> impact pages are planned.** They need the indexer's decoded tables, which do
> not exist yet.

## Routes

| Route | Behaviour | Depends on |
|---|---|---|
| `/` | Product summary, indexer health, explicit list of what is not built | `INDEXER_URL` |
| `/simulate` | Decodes a pasted base64 XDR `ScVal` in the browser with the pinned parser WASM; validates charset, 64 KiB cap and base64 framing before decoding | parser WASM |
| `/status` | The exact interface pins and `indexerHealth`, or the exact reason it is unavailable | `INDEXER_URL` |

## Error and staleness states

- **No fallback data.** `INDEXER_URL` has no default. Unset, malformed,
  credential-bearing, unreachable, non-JSON and schema-mismatched endpoints each
  render a specific reason.
- **Strict response validation.** `lib/graphql.ts` checks every field against
  the vendored SDL: integers must be safe non-negative integers, `healthy` must
  be a boolean, GraphQL `errors` are surfaced verbatim. Nothing is coerced or
  defaulted.
- **No cached health.** Health is requested per request
  (`dynamic = 'force-dynamic'`), so a stopped indexer cannot read as healthy.
- **No empty-but-fine tables.** Planned data pages will show an error state
  rather than an empty list that reads as "no data".

## Decode path

`/simulate` loads the `web` build of the pinned WASM from a vendored,
sha256-verified tarball. The same artifact, through its `nodejs` target, is the
decoder used by the Vitest suite — there is no mock decode path. Playwright
exercises the decode in real Chrome against the production build.

## Interface pins

| Artifact | Pin |
|---|---|
| Parser WASM | `v0.1.0-alpha.1`, sha256 `7b2eb942…` |
| GraphQL SDL | parser commit `ba3bf1a3…`, sha256 `cfbd85a8…` |

## Checks

| Command | Result recorded in the PR gate |
|---|---|
| `npm run check:pins` | vendored WASM and SDL match both locks |
| `npm run typecheck`, `npm run lint` | strict TypeScript, ESLint with the no-fixture-imports rule |
| `npm run test:unit` | 25 unit/component tests, real pinned WASM |
| `npm run build` | production build |
| `npx playwright test` | 4 e2e tests with `INDEXER_URL` deliberately unset |
| `bash scripts/check-claims.sh` | 7/7 README claims |

## Planned

- `/proposal/[id]`, `/delegates`, `/delegate/[address]`, execution impact with
  an **ESTIMATE at ledger N** badge.
- `graphql-codegen` from the vendored SDL, replacing the hand-written query.
- Event XDR decode, transaction-hash lookup and risk classification on
  `/simulate`, once the parser exposes those exports.
- A global staleness banner and visible `fixture-replay` vs `live` distinction.
- Lighthouse accessibility measurement, claimed after it is measured.
