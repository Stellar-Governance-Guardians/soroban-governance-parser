// Seed run state: contract ids, tx hashes, proposal records. Lives in the
// gitignored .seed/ directory; curated evidence is merged into the committed
// deployments.json by merge-deployments step after a verified run.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { STATE_FILE } from './config.js';

export function loadState() {
  if (!existsSync(STATE_FILE)) return null;
  return JSON.parse(readFileSync(STATE_FILE, 'utf8'));
}

export function saveState(state) {
  mkdirSync(dirname(STATE_FILE), { recursive: true });
  state.updatedAtUtc = new Date().toISOString();
  writeFileSync(STATE_FILE, `${JSON.stringify(state, null, 2)}\n`);
}

export function freshState() {
  return {
    $comment: 'Local seed-v2 state. Gitignored: contains no secrets, only addresses/hashes/tx evidence.',
    createdAtUtc: new Date().toISOString(),
    network: 'testnet',
    identities: {},
    wasms: {},
    contracts: {},
    proposals: {},
    steps: [],
  };
}
