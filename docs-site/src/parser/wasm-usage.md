# WASM usage

`crates/wasm` exposes the core crate to JavaScript through wasm-bindgen. The
package is published as a GitHub pre-release asset, `nodejs` and `web` targets
in one tarball, and consumed by content pin rather than by version range.

## Current release

| Field | Value |
|---|---|
| Version | `0.1.0-alpha.1` |
| Asset | `sgg-parser-wasm-0.1.0-alpha.1.tgz` |
| sha256 | `7b2eb9420f0cf0582d079f9603beb1e8548e1c7bac165724688cbae5b9237ecd` |
| Bytes | 245,210 |
| Targets | `nodejs`, `web` |
| Schema | `v1` |
| Toolchain | rustc 1.96.0 / wasm-pack 0.13.1 |

`releases/parser-wasm.lock.json` is the committed source of this table, and the
PR gate checks that the README, the lock file and the asset agree.

## Exports

| Export | Purpose |
|---|---|
| `decode_scval_base64(xdr_base64)` | `ScVal` → JSON string |
| `parse_contract_spec(wasm_bytes)` | extract and parse `contractspecv0` |
| `classify_risk(function_name, verified)` | apply the published rule table |
| `script3_decode_proposal(...)` | decode a Script3 proposal into actions |
| `script3_state_from_chain(...)` | reconstruct proposal state from chain reads |
| `script3_tally_from_chain(...)` | reconstruct the tally from chain reads |
| `evaluate_risk_context(call_json, risk_context_json)` | contextual rules over decoded arguments |
| `evaluate_tally(...)` | tally and quorum arithmetic |

There is **no event-normalization export** in this release: `matches_event`,
`normalize_event` and `decode_call` exist in the Rust core behind
`GovernorAdapter`, but the adapter trait is not exposed to WASM. Consumers that
need decoded events must wait for `v0.1.0-alpha.2`; see
[Honest limitations](../limitations.md).

## Node

```js
const wasm = require('./pkg/nodejs/sgp_parser_wasm.js');

const decoded = JSON.parse(wasm.decode_scval_base64('AAAADwAAABBwcm9wb3NhbF9jcmVhdGVk'));
// → "proposal_created"
```

## Browser

```js
import init, { decode_scval_base64 } from './pkg/web/sgp_parser_wasm.js';

await init();
const decoded = JSON.parse(decode_scval_base64('AAAAAwAAAAE='));
// → 1
```

The dashboard loads the `web` target from a hash-verified vendored tarball. Its
Content-Security-Policy allows `script-src 'self' 'wasm-unsafe-eval'`, which the
WASM instantiation requires.

## Tests over the artifact

Both targets are tested against the built package, not against a mock:

- `crates/wasm/js-tests/node.test.mjs` — Node test runner;
- `crates/wasm/js-tests/browser.test.mjs` — headless Chromium;
- the dashboard's Vitest suite loads the same artifact through its `nodejs`
  target, and its Playwright suite decodes in a real browser.

## Versioning

`v0.1.0-alpha.2` is the next release. It is expected to add the adapter event
surface (`matches_event` / `identify`, `normalize_event` with the `unverified`
fallback, `decode_call`) and the simulation request/response surface. Consumers
pin the asset by sha256, so the new package is a deliberate pin bump in each
consumer repository after the asset exists.
