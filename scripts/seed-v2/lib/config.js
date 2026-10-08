// Shared configuration for seed v2. Every value here is either read from
// committed registries (deployments.json, scripts/upstream/upstream.lock.json)
// or is an explicit, documented constant. No implicit defaults that could
// silently point at the wrong network (fail closed).
import { readFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
export const SEED_DIR = join(REPO_ROOT, '.seed');
export const KEYS_DIR = join(SEED_DIR, 'keys'); // stellar CLI --config-dir (gitignored)
export const STATE_FILE = join(SEED_DIR, 'state.json');
export const CAPTURE_DIR = join(SEED_DIR, 'capture');
export const FIXTURES_DIR = join(REPO_ROOT, 'tests', 'fixtures', 'seed-v2');
export const WASM_FIXTURES_DIR = join(FIXTURES_DIR, 'wasms');
export const DEPLOYMENTS_FILE = join(REPO_ROOT, 'deployments.json');
export const UPSTREAM_LOCK = join(REPO_ROOT, 'scripts', 'upstream', 'upstream.lock.json');

export function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

export function loadDeployments() {
  return readJson(DEPLOYMENTS_FILE);
}

export function loadUpstreamLock() {
  return readJson(UPSTREAM_LOCK);
}

export function loadState() {
  if (!existsSync(STATE_FILE)) return null;
  return readJson(STATE_FILE);
}

export function assertNetwork(deployments) {
  const net = deployments?.networks?.testnet;
  if (!net || net.rpcUrl !== 'https://soroban-testnet.stellar.org') {
    throw new Error('deployments.json does not point at the SDF public testnet RPC — refusing to run');
  }
  if (net.networkPassphrase !== 'Test SDF Network ; September 2015') {
    throw new Error('unexpected network passphrase — refusing to run');
  }
  return net;
}

// Governor schedule constants (see deployments.json / upstream.lock.json for bounds).
// Script3: MIN_VOTE_PERIOD = 720 ledgers (source-bound, enforced on-chain).
// OpenZeppelin: source bound is only "period != 0"; we deploy with 120 ledgers so
// a real multi-vote window exists (documented deviation — see fixtures README).
export const SCHEDULE = {
  script3: { voteDelay: 0, votePeriod: 720, timelock: 0, gracePeriod: 17280, quorumBps: 100, voteThresholdBps: 5000, countingType: 0b111, proposalThreshold: 1n },
  openzeppelin: { votingDelay: 0, votingPeriod: 120, proposalThreshold: 1n, quorum: 150_000_000_000n }, // 1% of 1_500_000e7 supply
};

// Five funded delegate identities + one deployer/council identity.
export const DELEGATE_IDS = ['sgg-delegate-1', 'sgg-delegate-2', 'sgg-delegate-3', 'sgg-delegate-4', 'sgg-delegate-5'];
export const DEPLOYER_ID = 'sgg-deployer';

// Voting power: raw i128 base units (7 decimals, matching both tokens).
export const DECIMALS = 7;
export const E7 = 10_000_000n;
export const MINTS = {
  'sgg-deployer': 50_000n * E7, // proposer rights for council-only Script3 actions (Upgrade)
  'sgg-delegate-1': 300_000n * E7,
  'sgg-delegate-2': 250_000n * E7,
  'sgg-delegate-3': 200_000n * E7,
  'sgg-delegate-4': 150_000n * E7,
  'sgg-delegate-5': 100_000n * E7,
  'sgg-governor-treasury': 500_000n * E7, // minted to each governor so an executable transfer exists
};

export const TRANSFER_AMOUNT = 1_000n * E7;
export const SUBCALL_AMOUNT = 10n * E7;
