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

// --- alpha.3 export surface -------------------------------------------------
// Every case below drives the *built* package. Event XDR is the committed
// Phase-1 capture `tests/fixtures/phase1-fixture-events.json` (vote_cast /
// proposal_created vectors) re-attributed to the registered Script3 governor
// contract, because `matches_event` correctly refuses a SAC contract id. The
// expected values are derived from that raw XDR and from the pinned upstream
// `contracts/governor/src/events.rs` layout, not from this implementation.

const GOVERNOR = 'CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX';
const SAC_CONTRACT = 'CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE';
const TX_HASH = 'ddef35404ed2fbef74c1e2324af631398aed9949ff2c7586d56d7cb17bfbea38';
const TOPIC_VOTE_CAST = 'AAAADwAAAAl2b3RlX2Nhc3QAAAA=';
const TOPIC_PROPOSAL_CREATED = 'AAAADwAAABBwcm9wb3NhbF9jcmVhdGVk';
const TOPIC_PROPOSAL_ID_1 = 'AAAAAwAAAAE=';
const TOPIC_VOTER = 'AAAAEgAAAAAAAAAAGAjYtOEbrkJZsPDJqVwqIg/Yz33P3732fRtY3s0mCAo=';
const VALUE_VOTE_CAST = 'AAAAEAAAAAEAAAACAAAAAwAAAAEAAAAJAAAAAAAAAAAAAAAAAA9CQA==';
const VALUE_PROPOSAL_CREATED_2_ELEMENTS =
  'AAAAEAAAAAEAAAACAAAADgAAABFGdW5kIHBhcnNlciBhdWRpdAAAAAAAAAkAAAAAAAAAAAAAAAAAD0JA';
const VOTE_CAST_TOPICS = JSON.stringify([TOPIC_VOTE_CAST, TOPIC_PROPOSAL_ID_1, TOPIC_VOTER]);

test('identify_governor returns the compiled adapter id', () => {
  assert.equal(wasm.identify_governor(), 'script3-soroban-governor');
});

test('matches_event accepts a full-topic Script3 vote_cast', () => {
  assert.equal(
    wasm.matches_event(GOVERNOR, 5035908, TX_HASH, VOTE_CAST_TOPICS, VALUE_VOTE_CAST),
    true,
  );
});

test('matches_event refuses a foreign contract id', () => {
  assert.equal(
    wasm.matches_event(SAC_CONTRACT, 5035908, TX_HASH, VOTE_CAST_TOPICS, VALUE_VOTE_CAST),
    false,
  );
});

test('matches_event refuses a topic-count mismatch', () => {
  // vote_cast declares exactly three topics; the full vector is required now.
  assert.equal(
    wasm.matches_event(
      GOVERNOR,
      5035908,
      TX_HASH,
      JSON.stringify([TOPIC_VOTE_CAST]),
      VALUE_VOTE_CAST,
    ),
    false,
  );
});

test('matches_event refuses a topic vector whose first entry is not a symbol', () => {
  assert.equal(
    wasm.matches_event(
      GOVERNOR,
      5035908,
      TX_HASH,
      JSON.stringify([TOPIC_PROPOSAL_ID_1, TOPIC_PROPOSAL_ID_1, TOPIC_VOTER]),
      VALUE_VOTE_CAST,
    ),
    false,
  );
});

test('matches_event errors on malformed XDR instead of returning false', () => {
  assert.throws(
    () => wasm.matches_event(GOVERNOR, 1, TX_HASH, JSON.stringify(['not-base64!!']), VALUE_VOTE_CAST),
    /invalid ScVal XDR/,
  );
  assert.throws(
    () => wasm.matches_event(GOVERNOR, 1, TX_HASH, VOTE_CAST_TOPICS, 'not-xdr'),
    /value is not valid ScVal XDR/,
  );
});

test('normalize_event decodes a matched vote_cast from captured XDR', () => {
  const event = JSON.parse(
    wasm.normalize_event(GOVERNOR, 5035908, TX_HASH, VOTE_CAST_TOPICS, VALUE_VOTE_CAST),
  );
  assert.equal(event.governor, 'script3-soroban-governor');
  assert.equal(event.decoding, 'verified');
  assert.equal(event.ledger, 5035908);
  assert.equal(event.tx_hash, TX_HASH);
  assert.equal(event.contract_id, GOVERNOR);
  assert.deepEqual(event.payload, {
    proposal_id: 1,
    // Derived from the topic XDR, not from this implementation: the third
    // topic is SCV_ADDRESS/account/ed25519 + the 32-byte key, StrKey-encoded.
    // See js-tests/expected-values.md for the bytes and the encoding steps.
    voter: 'GAMARWFU4EN24QSZWDYMTKK4FIRA7WGPPXH57PPWPUNVRXWNEYEAUP3C',
    support: 1,
    amount: '1000000',
  });
});

test('normalize_event refuses a matched event with an unexpected payload shape', () => {
  // proposal_created matches on topics (symbol + id + proposer) but its data
  // vector must hold five elements; the Phase-1 vector holds two.
  assert.throws(
    () =>
      wasm.normalize_event(
        GOVERNOR,
        5035906,
        TX_HASH,
        JSON.stringify([TOPIC_PROPOSAL_CREATED, TOPIC_PROPOSAL_ID_1, TOPIC_VOTER]),
        VALUE_PROPOSAL_CREATED_2_ELEMENTS,
      ),
    /proposal_created data must be 5 elements, got 2/,
  );
});

test('decode_call keeps the contract id and stays positional without a spec', () => {
  const call = JSON.parse(wasm.decode_call(GOVERNOR, 'vote', JSON.stringify([TOPIC_PROPOSAL_ID_1])));
  assert.equal(call.contract_id, GOVERNOR);
  assert.equal(call.function_name, 'vote');
  assert.deepEqual(call.arguments, [1]);
  assert.equal(call.decoding, 'unverified');
});

test('decode_call names arguments when the contract spec declares the function', () => {
  const specJson = wasm.parse_contract_spec(
    readFileSync(
      join(here, '..', '..', '..', 'tests', 'fixtures', 'seed-v2', 'wasms', 'script3-soroban-governor.wasm'),
    ),
  );
  const spec = JSON.parse(specJson);
  const fn = spec.functions.find((f) => f.name === 'get_proposal');
  assert.ok(fn, 'spec declares get_proposal');
  assert.equal(fn.inputs.length, 1);
  const call = JSON.parse(
    wasm.decode_call(GOVERNOR, 'get_proposal', JSON.stringify([TOPIC_PROPOSAL_ID_1]), specJson),
  );
  assert.equal(call.decoding, 'verified');
  assert.deepEqual(call.arguments, { [fn.inputs[0].name]: 1 });
  assert.equal(call.contract_id, GOVERNOR);
});

test('decode_call refuses a spec lookup for an undeclared function', () => {
  // The spec exists but does not declare this name: never invent argument names.
  const specJson = wasm.parse_contract_spec(
    readFileSync(
      join(here, '..', '..', '..', 'tests', 'fixtures', 'seed-v2', 'wasms', 'script3-soroban-governor.wasm'),
    ),
  );
  const call = JSON.parse(
    wasm.decode_call(GOVERNOR, 'not_a_governor_function', JSON.stringify([TOPIC_PROPOSAL_ID_1]), specJson),
  );
  assert.equal(call.decoding, 'unverified');
  assert.deepEqual(call.arguments, [1]);
});
