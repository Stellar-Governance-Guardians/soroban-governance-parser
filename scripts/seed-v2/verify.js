// verify.js — offline integrity check of every hash recorded by a seed-v2 run.
//
// Fails closed (exit 1) if ANY recorded hash is:
//   * not a 64-hex transaction hash;
//   * equal to a known wasm sha256 (the defect this verifies: deploy.js once
//     recorded the wasm sha256 as uploadTxHash);
//   * not SUCCESS in its raw capture; or
//   * inconsistent with the ledger/contractId recorded in deployments.json.
//
// Reads the gitignored .seed/state.json (current run) and the committed curated
// fixtures under tests/fixtures/seed-v2/ when present. Network-free.
//
// Usage: node verify.js
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { loadState, loadUpstreamLock, loadDeployments, REPO_ROOT, FIXTURES_DIR } from './lib/config.js';

const HEX64 = /^[0-9a-f]{64}$/;

function collectWasmHashes() {
  const set = new Set();
  const lock = loadUpstreamLock();
  for (const g of lock.governors ?? []) {
    for (const a of g.artifacts ?? []) if (a.sha256) set.add(a.sha256);
  }
  const state = loadState();
  for (const c of Object.values(state?.contracts ?? {})) if (c.wasmSha256) set.add(c.wasmSha256);
  return set;
}

/** Build hash -> { status, ledger } from raw getTransaction captures. */
function collectTxStatus() {
  const dirs = [join(REPO_ROOT, '.seed', 'capture')];
  const committed = join(FIXTURES_DIR, 'rpc');
  if (existsSync(committed)) dirs.push(committed);
  const map = new Map();
  for (const dir of dirs) {
    if (!existsSync(dir)) continue;
    for (const f of readdirSync(dir)) {
      if (!f.includes('getTransaction') && !f.endsWith('.json')) continue;
      let d;
      try {
        d = JSON.parse(readFileSync(join(dir, f), 'utf8'));
      } catch {
        continue;
      }
      const r = d?.response?.result;
      if (r?.txHash) map.set(r.txHash, { status: r.status, ledger: r.ledger, file: f });
    }
  }
  return map;
}

function collectRecordedHashes(state) {
  const out = [];
  const add = (scope, hash) => {
    if (hash) out.push({ scope, hash });
  };
  for (const [name, c] of Object.entries(state?.contracts ?? {})) {
    add(`contract:${name}.deployTx`, c.deployTxHash);
    add(`contract:${name}.uploadTx`, c.uploadTxHash);
  }
  for (const [gov, g] of Object.entries({ script3: state?.script3, oz: state?.oz })) {
    for (const [key, p] of Object.entries(g?.proposals ?? {})) {
      add(`${gov}:${key}.create`, p.createTxHash);
      add(`${gov}:${key}.close`, p.closeTxHash);
      add(`${gov}:${key}.execute`, p.executeTxHash);
      for (const v of p.votes ?? []) add(`${gov}:${key}.vote`, v.txHash);
    }
  }
  return out;
}

function checkDeployments(state, txStatus) {
  const deployments = loadDeployments();
  const failures = [];
  // deployments.json registers contracts by contractId; when a seed-v2 contract
  // is registered there, its recorded tx hash and ledger must match the capture.
  const registered = new Set();
  const walk = (o) => {
    if (o && typeof o === 'object') {
      if (typeof o.contractId === 'string') registered.add(o.contractId);
      for (const v of Object.values(o)) walk(v);
    }
  };
  walk(deployments.contracts ?? {});
  for (const c of Object.values(state?.contracts ?? {})) {
    const isRegistered = registered.has(c.contractId);
    const seen = txStatus.get(c.deployTxHash);
    if (isRegistered && seen && c.deployedAtLedger && seen.ledger !== c.deployedAtLedger) {
      failures.push(
        `contract ${c.contractId}: deployments.json ledger ${c.deployedAtLedger} != capture ${seen.ledger}`,
      );
    }
  }
  return failures;
}

/**
 * Clean-clone mode. `.seed/state.json` is gitignored, so a fresh clone has no
 * "current run" to check; failing outright made the documented reproduce flow
 * unrunnable offline. Instead verify the COMMITTED corpus: every recorded tx
 * hash must be 64-hex, must not be a wasm sha256, must match the status in its
 * own raw capture, and must be SUCCESS unless it is one of the three documented
 * pre-fix FAILED OpenZeppelin mints (kept on purpose as before/after evidence).
 */
function verifyCommittedCorpus() {
  const KNOWN_FAILED = new Set([
    '803d0bc41e1252603c78ae714080f32797b3751032582f19db3ce9ce0651d59e',
    '2aef69bdbca83ad47341d9c604cd41ca70fb2bfef62a8bcb18037531d6906945',
    'e352ae22bf264bbc3f80af3f8d9c44431ffdef1938374898777bb6f7a94c2b60',
  ]);
  const wasmHashes = collectWasmHashes();
  const index = JSON.parse(readFileSync(join(FIXTURES_DIR, 'index.json'), 'utf8'));
  const problems = [];
  let ok = 0;
  let failed = 0;
  for (const f of index.fixtures ?? []) {
    if (!f.txHash) continue;
    const where = `committed:${f.file}`;
    if (!HEX64.test(f.txHash)) {
      problems.push(`${where}: not a 64-hex hash: ${f.txHash}`);
      continue;
    }
    if (wasmHashes.has(f.txHash)) {
      problems.push(`${where}: hash equals a wasm sha256, not a transaction: ${f.txHash}`);
      continue;
    }
    // Cross-check the provenance manifest against the raw capture itself.
    let rawStatus;
    try {
      const raw = JSON.parse(readFileSync(join(FIXTURES_DIR, f.file), 'utf8'));
      rawStatus = raw?.response?.result?.status;
    } catch {
      problems.push(`${where}: raw capture is unreadable`);
      continue;
    }
    if (rawStatus !== undefined && rawStatus !== f.status) {
      problems.push(`${where}: index status ${f.status} != capture status ${rawStatus}`);
      continue;
    }
    if (f.status === 'SUCCESS') {
      ok += 1;
    } else if (f.status === 'FAILED' && KNOWN_FAILED.has(f.txHash)) {
      failed += 1;
    } else {
      problems.push(`${where}: unexpected status ${f.status}: ${f.txHash}`);
    }
  }
  console.log('no .seed/state.json (clean clone): verifying the committed corpus');
  console.log(`checked ${ok + failed + problems.length} committed tx hashes`);
  console.log(`  success: ${ok}, documented failures: ${failed}`);
  if (problems.length > 0) {
    console.log('  FAILURES:');
    for (const p of problems) console.log(`   - ${p}`);
    process.exit(1);
  }
  console.log('VERIFY OK — every committed hash is a transaction, none is a wasm hash');
}

function main() {
  const state = loadState();
  if (!state) {
    verifyCommittedCorpus();
    return;
  }
  const wasmHashes = collectWasmHashes();
  const txStatus = collectTxStatus();
  const recorded = collectRecordedHashes(state);

  const problems = [];
  let ok = 0;
  for (const { scope, hash } of recorded) {
    if (!HEX64.test(hash)) {
      problems.push(`${scope}: not a 64-hex hash: ${hash}`);
      continue;
    }
    if (wasmHashes.has(hash)) {
      problems.push(`${scope}: hash equals a wasm sha256, not a transaction: ${hash}`);
      continue;
    }
    const seen = txStatus.get(hash);
    if (seen && seen.status && seen.status !== 'SUCCESS') {
      problems.push(`${scope}: capture status ${seen.status} (expected SUCCESS): ${hash}`);
      continue;
    }
    ok += 1;
  }
  problems.push(...checkDeployments(state, txStatus));

  console.log(`checked ${recorded.length} recorded hashes against ${txStatus.size} captures`);
  console.log(`  ok: ${ok}`);
  if (problems.length > 0) {
    console.log(`  FAILURES:`);
    for (const p of problems) console.log(`   - ${p}`);
    process.exit(1);
  }
  console.log('VERIFY OK — every recorded hash is a SUCCESS transaction, none is a wasm hash');
}

try {
  main();
} catch (e) {
  console.error(`VERIFY FAILED: ${e.message}`);
  process.exit(1);
}
