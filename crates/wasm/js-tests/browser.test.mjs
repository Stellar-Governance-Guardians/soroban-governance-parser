// Headless-browser test for the wasm-pack `web` build.
//
// This is the browser half of the WASM release gate: it loads the built `web`
// package in a real headless Chromium (via Playwright), waits for the wasm to
// instantiate, and drives the same committed raw RPC captures the Rust
// differential tests use. A regression in the browser glue or the wasm surface
// fails here.
//
// Build first (web target), then run:
//   wasm-pack build crates/wasm --target web --out-dir pkg/web --out-name sgp_parser_wasm
//   npm i --no-save playwright@1.55.0 && npx playwright install --with-deps chromium
//   node crates/wasm/js-tests/browser.test.mjs
//
// No network beyond the local static server it starts. Fails closed if
// Playwright is not installed (that is a missing test dependency, not a skip).
import { createServer } from 'node:http';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, extname } from 'node:path';
import { createRequire } from 'node:module';
import assert from 'node:assert/strict';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);

let chromium;
try {
  ({ chromium } = require('playwright'));
} catch (e) {
  console.error('FAIL: playwright is required for the browser test:', e.message);
  process.exit(1);
}

const WEB_DIR = join(here, '..', 'pkg', 'web');
const RPC_DIR = join(here, '..', '..', '..', 'tests', 'fixtures', 'seed-v2', 'rpc');

if (!existsSync(join(WEB_DIR, 'sgp_parser_wasm.js'))) {
  console.error(`FAIL: web package not built at ${WEB_DIR} (run wasm-pack --target web first)`);
  process.exit(1);
}

function newestXdr(suffix) {
  const files = readdirSync(RPC_DIR)
    .filter((f) => /^\d/.test(f) && f.includes(suffix) && f.endsWith('.json'))
    .sort();
  assert.ok(files.length > 0, `no fixture matching ${suffix}`);
  const json = JSON.parse(readFileSync(join(RPC_DIR, files.at(-1)), 'utf8'));
  return json.response.result.results[0].xdr;
}

const MIME = {
  '.js': 'text/javascript',
  '.wasm': 'application/wasm',
  '.html': 'text/html',
  '.json': 'application/json',
};

const HARNESS = `<!doctype html><meta charset="utf-8">
<script type="module">
import init, {
  script3_state_from_chain, script3_tally_from_chain, evaluate_tally,
  decode_scval_base64, classify_risk, script3_decode_proposal,
  identify_governor, matches_event, normalize_event, decode_call,
} from './sgp_parser_wasm.js';
try {
  await init();
  window.wasm = {
    script3_state_from_chain, script3_tally_from_chain, evaluate_tally,
    decode_scval_base64, classify_risk, script3_decode_proposal,
    identify_governor, matches_event, normalize_event, decode_call,
  };
  window.__ready = true;
} catch (e) {
  window.__err = String(e);
  window.__ready = true;
}
</script>`;

const server = createServer((req, res) => {
  if (req.url === '/' || req.url === '/index.html') {
    res.writeHead(200, { 'content-type': 'text/html' });
    res.end(HARNESS);
    return;
  }
  const file = join(WEB_DIR, decodeURIComponent(req.url));
  try {
    const body = readFileSync(file);
    res.writeHead(200, { 'content-type': MIME[extname(file)] ?? 'application/octet-stream' });
    res.end(body);
  } catch {
    res.writeHead(404);
    res.end('not found');
  }
});

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

await new Promise((r) => server.listen(0, '127.0.0.1', r));
const port = server.address().port;
const browser = await chromium.launch();
const page = await browser.newPage();
const pageErrors = [];
page.on('pageerror', (e) => pageErrors.push(String(e)));

let failures = 0;
const check = (name, cond, detail = '') => {
  if (cond) {
    console.log(`ok   ${name}`);
  } else {
    failures += 1;
    console.error(`FAIL ${name}${detail ? `: ${detail}` : ''}`);
  }
};

try {
  await page.goto(`http://127.0.0.1:${port}/index.html`);
  await page.waitForFunction('window.__ready === true', null, { timeout: 30_000 });

  const bootErr = await page.evaluate(() => window.__err ?? null);
  check('wasm instantiates in the browser', bootErr === null, bootErr ?? '');
  check('no page errors', pageErrors.length === 0, pageErrors.join('; '));

  const stateJson = await page.evaluate(
    (xdr) => window.wasm.script3_state_from_chain(1, xdr),
    newestXdr('read-s3-get_proposal-s2-contract-upgrade'),
  );
  const state = JSON.parse(stateJson);
  check('state title', state.title === 'Seed v2: governor code upgrade', state.title);
  check('state vote_start', state.vote_start === 5083634, String(state.vote_start));
  check('state action variant', state.action.variant === 'upgrade', state.action.variant);

  const tallyJson = await page.evaluate(
    (xdr) => window.wasm.script3_tally_from_chain(xdr),
    newestXdr('read-s3-get_proposal_votes-s1-transfer-executed'),
  );
  const tally = JSON.parse(tallyJson);
  check('tally for_votes', tally.for_votes === 7_500_000_000_000, String(tally.for_votes));

  const outcomeJson = await page.evaluate(
    ([t, s]) => window.wasm.evaluate_tally(t, s, 15500000000000n),
    [tallyJson, SETTINGS],
  );
  check('tally outcome', JSON.parse(outcomeJson) === 'successful', outcomeJson);

  const symbol = await page.evaluate(() =>
    window.wasm.decode_scval_base64('AAAADwAAABBwcm9wb3NhbF9jcmVhdGVk'),
  );
  check('decode_scval_base64', JSON.parse(symbol) === 'proposal_created', symbol);

  const risk = JSON.parse(
    await page.evaluate(() => window.wasm.classify_risk('transfer', true)),
  );
  check('classify_risk tier', risk.tier === 'critical', risk.tier);

  // alpha.3 export surface: the same real seed-v2 vote_cast as the Node suite
  // (0066-getTransaction-final-s3-vote-...), re-attributed to the registered
  // governor contract id. The Phase-1 vector encodes the amount as U128, which
  // the adapter refuses, so it cannot serve as a positive case.
  const governorId = await page.evaluate(() => window.wasm.identify_governor());
  check('identify_governor', governorId === 'script3-soroban-governor', governorId);

  const voteCast = await page.evaluate(
    ([topic, id, voter]) =>
      window.wasm.matches_event(
        'CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX',
        5035908,
        'ddef35404ed2fbef74c1e2324af631398aed9949ff2c7586d56d7cb17bfbea38',
        JSON.stringify([topic, id, voter]),
        'AAAAEAAAAAEAAAACAAAAAwAAAAEAAAAKAAAAAAAAAAAAAAK6fe8wAA==',
      ),
    ['AAAADwAAAAl2b3RlX2Nhc3QAAAA=', 'AAAAAwAAAAA=', 'AAAAEgAAAAAAAAAAzT8nsj+8SDy+0Q+IpClijkUvvaZ8oVUmL8Vs4A=='],
  );
  check('matches_event (full topic vector)', voteCast === true, String(voteCast));

  const foreign = await page.evaluate(
    ([topic, id, voter]) =>
      window.wasm.matches_event(
        'CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE',
        5035908,
        'x',
        JSON.stringify([topic, id, voter]),
        'AAAAEAAAAAEAAAACAAAAAwAAAAEAAAAKAAAAAAAAAAAAAAK6fe8wAA==',
      ),
    ['AAAADwAAAAl2b3RlX2Nhc3QAAAA=', 'AAAAAwAAAAA=', 'AAAAEgAAAAAAAAAAzT8nsj+8SDy+0Q+IpClijkUvvaZ8oVUmL8Vs4A=='],
  );
  check('matches_event refuses a foreign contract', foreign === false, String(foreign));

  const normalized = JSON.parse(
    await page.evaluate(
      ([topic, id, voter]) =>
        window.wasm.normalize_event(
          'CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX',
          5035908,
          'ddef35404ed2fbef74c1e2324af631398aed9949ff2c7586d56d7cb17bfbea38',
          JSON.stringify([topic, id, voter]),
          'AAAAEAAAAAEAAAACAAAAAwAAAAEAAAAKAAAAAAAAAAAAAAK6fe8wAA==',
        ),
      ['AAAADwAAAAl2b3RlX2Nhc3QAAAA=', 'AAAAAwAAAAA=', 'AAAAEgAAAAAAAAAAzT8nsj+8SDy+0Q+IpClijkUvvaZ8oVUmL8Vs4A=='],
    ),
  );
  check('normalize_event amount', normalized.payload.amount === '3000000000000', normalized.payload.amount);
  check('normalize_event decoding', normalized.decoding === 'verified', normalized.decoding);

  const call = JSON.parse(
    await page.evaluate(() =>
      window.wasm.decode_call(
        'CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX',
        'vote',
        JSON.stringify(['AAAAAwAAAAE=']),
      ),
    ),
  );
  check('decode_call keeps contract id', call.contract_id.startsWith('CCKGJBCB'), call.contract_id);
  check('decode_call spec-less is positional', call.decoding === 'unverified' && call.arguments[0] === 1, JSON.stringify(call.arguments));
} finally {
  await browser.close();
  server.close();
}

if (failures > 0) {
  console.error(`\n${failures} browser assertion(s) failed`);
  process.exit(1);
}
console.log('\nbrowser wasm test: OK');
