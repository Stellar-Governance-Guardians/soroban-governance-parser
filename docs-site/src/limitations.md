# Honest limitations

Everything on this page is a gap, not a claim. If a feature appears elsewhere on
the site, it is either implemented and checked, or it carries a **Status:
planned** banner. Where a gap blocks a milestone, the blocker is named.

## Parser

| Limitation | Detail |
|---|---|
| OpenZeppelin adapter not implemented | Research, committed test contracts and fixtures exist; no adapter decodes OZ proposals or events. `claims.json` asserts this state (`oz-adapter-not-implemented`). |
| WASM event surface | `matches_event`, `normalize_event` and `decode_call` are exported in `v0.1.0-alpha.2`; the indexer still needs persisted decode tables. |
| CLI simulation boundary | `sgp simulate` accepts a validated transaction envelope and emits an estimate; proposal-action assembly and rent remain out of scope. |
| Rent is not modelled | The committed `simulateTransaction` captures contain no rent figure; the field is omitted rather than estimated from an assumption. |
| Dry-run results are estimates | `estimate: true` and `simulatedAtLedger` are mandatory. A simulation runs against the current ledger, not the future execution ledger. |
| No mainnet DAO exercised | Fixtures come from testnet deployments listed in [Evidence](generated/evidence.md). |
| TLS advisories fixed by patch update | `cargo audit` reported six advisories in `rustls` 0.23.45, `rustls-webpki` 0.103.15 and `webpki-root-certs` 1.0.9. Updated to 0.23.46, 0.103.16 and 1.0.11; the nightly audit job passes. One unsound-API warning (`tokio`, RUSTSEC-2026-0386) remains a warning with no reported patched release. |
| Expired live fixtures need the deployer key | Six seed-v2 contracts deployed on 2026-10-08 answer with `liveUntilLedger: 0`. The live tier reports each one as a warning with the re-seed command; refreshing them needs the funded deployer account, which CI does not hold. The phase-1 fixture governor and the seed-v2 governor remain live. |

## Indexer

| Limitation | Detail |
|---|---|
| No proposal-level decode | Event normalization is persisted through the pinned WASM; proposal/action projections remain planned. |
| GraphQL surface scope | `/graphql` exposes health and a raw-event connection; proposal, impact, and delegate fields remain unimplemented. |
| Health surface is narrow | `/health` fails on open ingestion gaps, but reconciliation drift checks arrive with I5. |
| No reconciliation | SQL tallies have not been compared against on-chain reads (I5). |
| No deploy, no soak | The Docker/compose recipe and `deploy.yml` (I6) ship, but no instance has been deployed and no 72-hour run has happened. |
| Indexer prove is fixture-only | The indexer's clean-clone proof (I7) replays committed fixtures and asserts the GraphQL values; it has not been run against a live indexer or a live ledger. |
| Integration tests need Postgres | `npm test` fails loudly without `DATABASE_URL` instead of skipping; CI provides a service container. |
| Testnet resets | The public testnet is reset periodically; pinned artifacts and contract ids can become unreachable. Health and gap reporting are designed to make that visible. |

## Dashboard

| Limitation | Detail |
|---|---|
| Data pages not built | `/proposal/[id]`, `/delegates`, `/delegate/[address]` and execution-impact panels need the indexer's decoded tables. |
| No codegen | The single query is hand-written and strictly validated; `graphql-codegen` waits for the full schema to be served. |
| `/simulate` decodes one `ScVal` | Event XDR, transaction-hash lookup, proposal call decode and risk flags are not wired into the dashboard yet. |
| No live-indexer run | No page has been exercised end to end against a deployed indexer; the local GraphQL contract now exists. |
| Accessibility checked structurally, not measured | Landmarks, skip link, labelled controls and an `aria-live` region are in place; no Lighthouse score has been measured and none is claimed. |
| CSP uses `'unsafe-inline'` | Required for Next's hydration bootstrap today; a nonce-based policy is planned. |

## This site

| Limitation | Detail |
|---|---|
| Sibling prose is summarised, not embedded | The indexer and dashboard pages are parser-repo summaries written against pinned revisions recorded in `sources.lock`. They are not the sibling repositories' own documentation, and they will lag until the pin is bumped each phase. |
| External links checked nightly | The PR gate builds and link-checks offline; live URL checks (including the deployed site) run in the nightly tier and never block a merge. |
| No translation | The site is English. |
