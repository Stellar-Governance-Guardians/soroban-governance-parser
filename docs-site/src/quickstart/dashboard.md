# Dashboard quickstart

Next.js 16 App Router, strict TypeScript, Tailwind v4. Node ≥ 24.

## Run it

```bash
git clone https://github.com/Stellar-Governance-Guardians/delegate-portal-dashboard
cd delegate-portal-dashboard
npm ci
npm run dev     # http://localhost:3000
```

The `predev`/`prebuild`/`pretest:unit` hooks run `scripts/unpack-wasm.mjs`,
which verifies the vendored parser WASM tarball against `parser.lock` and
unpacks it into `generated/wasm/`. No network access is needed.

To show indexer data, point it at a GraphQL endpoint:

```bash
INDEXER_URL=http://127.0.0.1:4000/graphql npm run dev
```

`INDEXER_URL` has no default. Unset, malformed or unreachable, every data
surface renders the exact reason instead of an empty table.

## Checks

```bash
npm run check:pins     # vendored WASM + SDL vs the lock files
npm run typecheck
npm run lint
npm run test:unit      # 25 unit/component tests, real pinned WASM, no mock decode
npm run build
npx playwright test    # 4 e2e tests against `next build && next start`
bash scripts/check-claims.sh   # 7/7 README claims
```

The e2e run deliberately leaves `INDEXER_URL` unset, so it proves the
unavailable-indexer state rather than a happy path that needs a live server.

## Routes

| Route | Behaviour |
|---|---|
| `/` | Product summary, indexer health, explicit list of what is not built |
| `/simulate` | Decodes a pasted base64 XDR `ScVal` in the browser with the pinned WASM |
| `/status` | The exact interface pins and `indexerHealth`, or the exact reason it is unavailable |

Proposal, delegate and impact pages are planned; they need the indexer's
decoded tables.
