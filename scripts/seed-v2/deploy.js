// deploy.js — deploy the pinned upstream governor stack on live testnet.
//
// Idempotent: re-running adopts already-deployed contracts from .seed/state.json
// (verified alive) and only deploys what is missing. Every deploy transaction is
// captured raw via getTransaction. Contract constructor arguments come from
// upstream source (read at the pinned SHAs — see scripts/upstream/upstream.lock.json).
//
// Usage: node deploy.js
import { createHash } from 'node:crypto';
import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { stellar, extractSigningHashes, extractExplorerTxHashes, extractContractId } from './lib/cli.js';
import { ensureIdentity, addressOf } from './lib/keys.js';
import { loadUpstreamLock, WASM_FIXTURES_DIR, DEPLOYER_ID, DELEGATE_IDS, SCHEDULE, KEYS_DIR } from './lib/config.js';
import { loadState, freshState, saveState } from './lib/state.js';
import { rpcRaw, health } from './lib/rpc.js';

const KEY = ['--config-dir', KEYS_DIR];
const NET = ['--network', 'testnet'];

// Fixture wasm file name -> artifact key in upstream.lock.json.
const WASMS = {
  script3Votes: 'script3-soroban-votes.wasm',
  script3Governor: 'script3-soroban-governor.wasm',
  script3MockSubcall: 'script3-mock-subcall.wasm',
  ozToken: 'oz-fungible-governor-token.wasm',
  ozGovernor: 'oz-fungible-governor-contract.wasm',
  ozUpgradeableV1: 'oz-upgradeable-v1-example.wasm',
};

function sha256File(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function verifyWasms(lock) {
  const artifacts = lock.governors.flatMap((g) => g.artifacts);
  for (const [name, file] of Object.entries(WASMS)) {
    const path = join(WASM_FIXTURES_DIR, file);
    if (!existsSync(path)) {
      throw new Error(`missing committed wasm fixture ${file} — run scripts/upstream/build-upstream.sh first`);
    }
    const actual = sha256File(path);
    const pinned = artifacts.find((a) => a.file === file);
    if (!pinned) throw new Error(`wasm ${file} is not pinned in upstream.lock.json (fail closed)`);
    if (pinned.sha256 !== actual) {
      throw new Error(`wasm ${file} sha256 ${actual} != pinned ${pinned.sha256} (fail closed)`);
    }
    console.log(`  ok ${name}: ${file} sha256=${actual.slice(0, 16)}…`);
  }
}

function deployContract({ state, name, wasmKey, ctorArgs = [] }) {
  const existing = state.contracts[name];
  if (existing?.contractId) {
    console.log(`  skip ${name}: already deployed ${existing.contractId}`);
    return existing;
  }
  const file = join(WASM_FIXTURES_DIR, WASMS[wasmKey]);
  const args = [
    'contract', 'deploy', '--wasm', file,
    '--source-account', DEPLOYER_ID, ...NET, ...KEY, '--',
    ...ctorArgs,
  ];
  const { out, stdout } = stellar(args, { logName: `deploy-${name}` });
  const contractId = extractContractId(stdout) ?? extractContractId(out);
  if (!contractId) throw new Error(`deploy(${name}): no contract id in output:\n${out}`);
  // Parse only structured signals. NEVER scan for "any 64-hex string": the CLI
  // prints the wasm sha256 (`Deploying contract using wasm hash <sha>`) which is
  // not a transaction hash. Submission order is: optional upload tx, then the
  // contract-create tx. When the wasm was already installed the CLI says
  // "Skipping install…" and emits only the create tx.
  const signed = extractSigningHashes(out);
  const explorer = extractExplorerTxHashes(out);
  const txHashes = explorer.length > 0 ? explorer : signed;
  const skippedInstall = /Skipping install because wasm already installed/.test(out);
  const deployTxHash = txHashes.length >= 1 ? txHashes[txHashes.length - 1] : null;
  const uploadTxHash = !skippedInstall && txHashes.length >= 2 ? txHashes[0] : null;
  if (!deployTxHash) {
    throw new Error(`deploy(${name}): no transaction hash found in CLI output (fail closed):\n${out}`);
  }
  const record = {
    contractId,
    wasmKey,
    wasmFile: WASMS[wasmKey],
    wasmSha256: sha256File(file),
    deployTxHash,
    uploadTxHash,
    constructorArgs: ctorArgs.length ? ctorArgs.join(' ') : null,
    deployedAtUtc: new Date().toISOString(),
  };
  state.contracts[name] = record;
  saveState(state);
  console.log(`  deployed ${name} -> ${contractId} (tx ${deployTxHash})`);
  return record;
}

async function verifyDeployTxs(state) {
  for (const [name, rec] of Object.entries(state.contracts)) {
    if (!rec.deployTxHash) continue;
    const got = await rpcRaw('getTransaction', { hash: rec.deployTxHash }, { note: `deploy-${name}` });
    const r = got.result;
    if (!r || r.status !== 'SUCCESS') {
      throw new Error(`deploy tx for ${name} not SUCCESS: ${JSON.stringify(got).slice(0, 400)}`);
    }
    rec.deployedAtLedger = r.ledger;
    console.log(`  verified ${name} deploy tx ${rec.deployTxHash} @ ledger ${r.ledger}`);
  }
}

async function main() {
  console.log('== seed v2: deploy ==');
  await health('deploy-start');

  const lock = loadUpstreamLock();
  console.log('== verifying pinned wasm fixtures ==');
  verifyWasms(lock);

  console.log('== identities ==');
  const state = loadState() ?? freshState();
  ensureIdentity(DEPLOYER_ID);
  for (const d of DELEGATE_IDS) ensureIdentity(d);
  state.identities[DEPLOYER_ID] = addressOf(DEPLOYER_ID);
  for (const d of DELEGATE_IDS) state.identities[d] = addressOf(d);
  saveState(state);
  console.log(`  deployer ${state.identities[DEPLOYER_ID]} (council for Script3)`);

  console.log('== contracts ==');
  deployContract({ state, name: 'script3Votes', wasmKey: 'script3Votes' });
  deployContract({ state, name: 'script3Governor', wasmKey: 'script3Governor' });
  deployContract({ state, name: 'script3MockSubcall', wasmKey: 'script3MockSubcall' });
  deployContract({ state, name: 'ozToken', wasmKey: 'ozToken', ctorArgs: ['--owner', state.identities[DEPLOYER_ID]] });
  const ozGovernorCtor = [
    '--token-contract', state.contracts.ozToken?.contractId ?? '',
    '--voting-delay', String(SCHEDULE.openzeppelin.votingDelay),
    '--voting-period', String(SCHEDULE.openzeppelin.votingPeriod),
    '--proposal-threshold', String(SCHEDULE.openzeppelin.proposalThreshold),
    '--quorum', String(SCHEDULE.openzeppelin.quorum),
  ];
  if (!ozGovernorCtor[1]) throw new Error('ozToken not deployed (fail closed)');
  deployContract({ state, name: 'ozGovernor', wasmKey: 'ozGovernor', ctorArgs: ozGovernorCtor });
  deployContract({
    state, name: 'ozUpgradeableV1', wasmKey: 'ozUpgradeableV1',
    ctorArgs: ['--admin', state.identities[DEPLOYER_ID], '--rate', '100'],
  });
  deployContract({ state, name: 'ozMockSubcall', wasmKey: 'script3MockSubcall' });

  console.log('== verifying deploy transactions on-chain ==');
  await verifyDeployTxs(state);
  saveState(state);

  console.log('== deployed contracts ==');
  for (const [name, rec] of Object.entries(state.contracts)) {
    console.log(`  ${name.padEnd(20)} ${rec.contractId}  tx ${rec.deployTxHash ?? 'unknown'}  ledger ${rec.deployedAtLedger ?? '?'}`);
  }
  console.log('DEPLOY OK');
}

main().catch((e) => {
  console.error(`DEPLOY FAILED: ${e.message}`);
  process.exit(1);
});
