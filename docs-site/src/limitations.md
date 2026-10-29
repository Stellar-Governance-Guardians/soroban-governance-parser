# Honest limitations

Everything on this page is a gap, not a claim. If a feature appears elsewhere on
the site, it is either implemented and checked, or it carries a **Status:
planned** banner. Where a gap blocks a milestone, the blocker is named.

## Parser

| Limitation | Detail |
|---|---|
| OpenZeppelin adapter not implemented | Research, committed test contracts and fixtures exist; no adapter decodes OZ proposals or events. `claims.json` asserts this state (`oz-adapter-not-implemented`). |
| No WASM event surface | `matches_event`, `normalize_event` and `decode_call` exist in Rust behind `GovernorAdapter` but are not exported. Planned for `v0.1.0-alpha.2`. |
| No CLI `simulate` command | The dry-run model is exercised by offline tests against committed captures; an operator cannot run it from `sgp` yet. |
| Rent is not modelled | The committed `simulateTransaction` captures contain no rent figure; the field is omitted rather than estimated from an assumption. |
| Dry-run results are estimates | `estimate: true` and `simulatedAtLedger` are mandatory. A simulation runs against the current ledger, not the future execution ledger. |
| No mainnet DAO exercised | Fixtures come from testnet deployments listed in [Evidence](generated/evidence.md). |
| Dependency advisories open | `cargo audit` currently reports advisories in transitive TLS/async crates (`rustls`, `rustls-webpki`, `webpki-root-certs`, an unsound `tokio` API warning). Not yet fixed; the nightly audit job is red until the lockfile is refreshed. |
| Live-testnet claims can fail silently in CI | The nightly `live-checks` workflow currently fails at its online-claims step. Its logs have expired, so the failing claim is unidentified and must be re-run before any live result is quoted. |

## Indexer

| Limitation | Detail |
|---|---|
| No decode | Raw events are stored; nothing interprets them yet (I3). The pinned WASM is hash-verified but unused. |
| No GraphQL API | No endpoint exists (I4). The [API reference](generated/api.md) is the schema the server must implement. |
| No health surface | `ingest_gaps` rows are written and queryable, but there is no `/health` that fails on open gaps; it arrives with I5. |
| No reconciliation | SQL tallies have not been compared against on-chain reads (I5). |
| No deploy, no soak | No Docker/compose recipe (I6) and no 72-hour run. Nothing has been deployed. |
| No clean-clone prove | `scripts/prove-indexer` does not exist (I7). |
| Integration tests need Postgres | `npm test` fails loudly without `DATABASE_URL` instead of skipping; CI provides a service container. |
| Testnet resets | The public testnet is reset periodically; pinned artifacts and contract ids can become unreachable. Health and gap reporting are designed to make that visible. |

## Dashboard

| Limitation | Detail |
|---|---|
| Data pages not built | `/proposal/[id]`, `/delegates`, `/delegate/[address]` and execution-impact panels need the indexer's decoded tables. |
| No codegen | The single query is hand-written and strictly validated; `graphql-codegen` waits for the full schema to be served. |
| `/simulate` decodes one `ScVal` | Event XDR, transaction-hash lookup, proposal call decode and risk flags need parser exports that are not in `v0.1.0-alpha.1`. |
| No live-indexer run | No page has been exercised end to end against a running indexer, because no indexer API exists. |
| Accessibility checked structurally, not measured | Landmarks, skip link, labelled controls and an `aria-live` region are in place; no Lighthouse score has been measured and none is claimed. |
| CSP uses `'unsafe-inline'` | Required for Next's hydration bootstrap today; a nonce-based policy is planned. |

## This site

| Limitation | Detail |
|---|---|
| Sibling prose is summarised, not embedded | The indexer and dashboard pages are parser-repo summaries written against pinned revisions recorded in `sources.lock`. They are not the sibling repositories' own documentation, and they will lag until the pin is bumped each phase. |
| External links checked nightly | The PR gate builds and link-checks offline; live URL checks (including the deployed site) run in the nightly tier and never block a merge. |
| No translation | The site is English. |
