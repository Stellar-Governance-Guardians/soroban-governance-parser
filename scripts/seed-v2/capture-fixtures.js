// capture-fixtures.js — curate raw RPC captures into committed fixtures.
//
// Charter rule 3: every committed fixture is a VERBATIM raw RPC response with
// recorded provenance, so the offline differential tests survive testnet RPC
// retention expiry (120,960 ledgers). Nothing is written back to .seed/: we only
// read the raw capture log and copy responses into tests/fixtures/seed-v2/.
//
// The raw captures live in .seed/capture/ (gitignored): keys, signed envelopes
// and the full log never leave the machine. Committed fixtures are the subset
// needed as evidence plus an index.json manifest.
//
// Usage: node capture-fixtures.js
import { readFileSync, writeFileSync, mkdirSync, existsSync, copyFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { CAPTURE_DIR, FIXTURES_DIR } from './lib/config.js';

const OUT_RPC = join(FIXTURES_DIR, 'rpc');
const MANIFEST = join(FIXTURES_DIR, 'index.json');

// RPC methods whose responses are committed as evidence. Everything else
// (sendTransaction, getAccount, …) stays only in the gitignored .seed/.
const KEEP = new Set(['getHealth', 'getTransaction', 'getEvents', 'getLedgerEntries', 'simulateTransaction']);

function readIndex() {
  const p = join(CAPTURE_DIR, 'index.jsonl');
  if (!existsSync(p)) throw new Error(`no capture index at ${p} — run deploy.js/seed.js/settle.js first`);
  return readFileSync(p, 'utf8')
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => JSON.parse(l));
}

/** Extract provenance fields from a capture response, when present. */
function derive(response) {
  const r = response?.result ?? null;
  if (!r || typeof r !== 'object') return {};
  if (r.txHash) return { txHash: r.txHash, status: r.status ?? null, ledger: r.ledger ?? null };
  if (r.latestLedger && r.ledgerRetentionWindow) {
    return { latestLedger: r.latestLedger, ledgerRetentionWindow: r.ledgerRetentionWindow };
  }
  if (Array.isArray(r.events)) return { eventCount: r.events.length };
  if (Array.isArray(r.entries)) return { entryCount: r.entries.length };
  return {};
}

function main() {
  // Clear the output dir so committed fixtures exactly match the current run
  // (stale files from an earlier run must not linger as apparent evidence).
  rmSync(OUT_RPC, { recursive: true, force: true });
  mkdirSync(OUT_RPC, { recursive: true });
  const index = readIndex();
  const manifest = {
    $comment:
      'Provenance for committed seed-v2 fixtures. Each entry is a verbatim raw Soroban RPC response copied from .seed/capture/ (gitignored). Regenerate with `node capture-fixtures.js` after a live run.',
    generatedAtUtc: new Date().toISOString(),
    source: '.seed/capture',
    fixtures: [],
  };

  let kept = 0;
  for (const entry of index) {
    const method = entry.method.replace(/-\d+$/, '');
    if (!KEEP.has(method)) continue;
    const src = join(CAPTURE_DIR, entry.file);
    if (!existsSync(src)) continue;
    const payload = JSON.parse(readFileSync(src, 'utf8'));
    copyFileSync(src, join(OUT_RPC, entry.file));
    manifest.fixtures.push({
      file: `rpc/${entry.file}`,
      method: entry.method,
      note: entry.note || '',
      capturedAtUtc: payload.capturedAtUtc ?? null,
      ...derive(payload.response),
    });
    kept += 1;
  }

  writeFileSync(MANIFEST, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`wrote ${kept} fixtures + ${MANIFEST}`);
}

main();
