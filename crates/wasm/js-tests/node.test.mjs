// Node smoke test for the wasm-pack `nodejs` build of soroban-governance-wasm.
//
// This is the Node half of the WASM release gate (charter: test the delivered
// artifact through the interface the consumer uses). It imports the *built*
// package — not the Rust source — and exercises decode / risk / tally against
// the same committed raw RPC captures the Rust differential tests use, so a
// regression in the wasm surface or the generated glue fails here.
//
// Run after building the nodejs target:
//   wasm-pack build crates/wasm --target nodejs --out-dir pkg/nodejs --out-name sgp_parser_wasm
//   node --test crates/wasm/js-tests/node.test.mjs
//
// No network: it reads committed fixtures only.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);
const wasm = require(join(here, '..', 'pkg', 'nodejs', 'sgp_parser_wasm.js'));

const RPC_DIR = join(here, '..', '..', '..', 'tests', 'fixtures', 'seed-v2', 'rpc');

/** Newest committed capture whose filename contains `suffix` (mirrors the Rust test). */
function newestXdr(suffix) {
  const files = readdirSync(RPC_DIR)
    .filter((f) => /^\d/.test(f) && f.includes(suffix) && f.endsWith('.json'))
    .sort();
  assert.ok(files.length > 0, `no fixture matching ${suffix}`);
  const json = JSON.parse(readFileSync(join(RPC_DIR, files.at(-1)), 'utf8'));
  const xdr = json?.response?.result?.results?.[0]?.xdr;
  assert.equal(typeof xdr, 'string', `fixture ${suffix} has no results[0].xdr`);
  return xdr;
}

const SETTINGS = JSON.stringify({
  quorum_bps: 100,
  vote_threshold_bps: 5000,
  counting_type: 7,
  proposal_threshold: 1,
  vote_delay: 0,
  vote_period: 720,
  timelock: 0,
  grace_period: 17280,
});
const TOTAL_SUPPLY = 15_500_000_000_000n;

test('script3_state_from_chain decodes a real captured get_proposal', () => {
  const state = JSON.parse(
    wasm.script3_state_from_chain(1, newestXdr('read-s3-get_proposal-s2-contract-upgrade')),
  );
  assert.equal(state.governor, 'script3-soroban-governor');
  assert.equal(state.proposal_id, 1);
  assert.equal(state.title, 'Seed v2: governor code upgrade');
  assert.equal(state.vote_start, 5083634);
  assert.equal(state.vote_end, 5084354);
  assert.equal(state.action.variant, 'upgrade');
});

test('script3_tally_from_chain decodes a real captured get_proposal_votes', () => {
  const tally = JSON.parse(
    wasm.script3_tally_from_chain(newestXdr('read-s3-get_proposal_votes-s1-transfer-executed')),
  );
  assert.equal(tally.for_votes, 7_500_000_000_000);
  assert.equal(tally.against, 0);
  assert.equal(tally.abstain, 0);
});

test('evaluate_tally reproduces the on-chain verdict', () => {
  const tally = wasm.script3_tally_from_chain(
    newestXdr('read-s3-get_proposal_votes-s1-transfer-executed'),
  );
  // i128 crosses the wasm-bindgen boundary as BigInt, not Number.
  const outcome = JSON.parse(wasm.evaluate_tally(tally, SETTINGS, TOTAL_SUPPLY));
  assert.equal(outcome, 'successful');
});

test('decode_scval_base64 and classify_risk are exposed and fail closed', () => {
  // "proposal_created" symbol XDR (the README quick-start vector); decode_scval_base64
  // returns a JSON string, so the symbol comes back quoted.
  assert.equal(
    JSON.parse(wasm.decode_scval_base64('AAAADwAAABBwcm9wb3NhbF9jcmVhdGVk')),
    'proposal_created',
  );
  const risk = JSON.parse(wasm.classify_risk('transfer', true));
  assert.equal(risk.tier, 'critical');
  assert.throws(() => wasm.decode_scval_base64('not-xdr'), /XDR/i);
});

test('decode_proposal accepts an empty RiskContext (documented default)', () => {
  // Round-trip the decoded s2 state through decode_proposal with default context.
  const state = wasm.script3_state_from_chain(
    1,
    newestXdr('read-s3-get_proposal-s2-contract-upgrade'),
  );
  const proposal = JSON.parse(wasm.script3_decode_proposal(state, '{}'));
  assert.equal(proposal.governor, 'script3-soroban-governor');
  assert.equal(proposal.calls[0].function_name, 'upgrade');
});
