// Offline unit tests for the seed-v2 CLI output parsers. No network, no keys.
//
// Regression guard for the uploadTxHash defect: the previous code scanned for
// "any 64-hex string", so the CLI's `Deploying contract using wasm hash <sha>`
// line could be recorded as a transaction hash. These fixtures reproduce the
// exact shapes observed in .seed/capture/cli/*.log (2026-10-07).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  extractSigningHashes,
  extractExplorerTxHashes,
  extractContractId,
} from '../lib/cli.js';

const WASM_SHA = 'e8bbc883b2c7fcd8716b07271929eca801fb40f2d3cf5964b139fa741c40f787';
const DEPLOY_TX = '030940285cd226e24f5971ae7a1a3e88ad50c0cb612d2575ac060148318f9141';

// Case A: wasm already installed ("Skipping install…") => a single (create) tx.
const SKIPPED_INSTALL_LOG = `$ stellar contract deploy --wasm .../script3-soroban-votes.wasm --source-account sgg-deployer --network testnet --config-dir ... --

CBLEYZZL3LRZO2O4EU6S2JE72H6ZIYREPSDZCD7QXP3V45FDHCKI63JQ
ℹ️  Uploading contract WASM…
ℹ️  Skipping install because wasm already installed
ℹ️  Deploying contract using wasm hash ${WASM_SHA}
ℹ️  Simulating transaction…
ℹ️  Signing transaction: ${DEPLOY_TX}
🌎 Sending transaction…
✅ Transaction submitted successfully!
🔗 https://stellar.expert/explorer/testnet/tx/${DEPLOY_TX}
🔗 https://lab.stellar.org/r/testnet/contract/CBLEYZZL3LRZO2O4EU6S2JE72H6ZIYREPSDZCD7QXP3V45FDHCKI63JQ
✅ Deployed!
`;

const UPLOAD_TX = '8840bd6bec431e8ec56b7ac8b19126dfa9ab5ed63a3104ae3eb7162fb85f80c2';
const GOV_WASM_SHA = '16033b38b740b1d5af82025a671490b3cea2a0df59b7ffad005f3f7ca3fa27f0';
const GOV_DEPLOY_TX = 'e808b1d44fc16e7b50b3b986d6da8c4e9b4de7a33114ab063b03d574a2e79324';

// Case B: fresh upload => upload tx then create tx.
const UPLOAD_LOG = `$ stellar contract deploy --wasm .../script3-soroban-governor.wasm ...
CBZKQQDZLYN4DLLHCE5JCOV2XVM3X6YU6PLSERGZBOGAT3J6AKRNAEEP
ℹ️  Uploading contract WASM…
ℹ️  Simulating transaction…
ℹ️  Signing transaction: ${UPLOAD_TX}
🌎 Sending transaction…
✅ Transaction submitted successfully!
🔗 https://stellar.expert/explorer/testnet/tx/${UPLOAD_TX}
ℹ️  Deploying contract using wasm hash ${GOV_WASM_SHA}
ℹ️  Simulating transaction…
ℹ️  Signing transaction: ${GOV_DEPLOY_TX}
🌎 Sending transaction…
✅ Transaction submitted successfully!
🔗 https://stellar.expert/explorer/testnet/tx/${GOV_DEPLOY_TX}
✅ Deployed!
`;

test('skipped-install deploy yields one tx hash and no upload hash', () => {
  const signed = extractSigningHashes(SKIPPED_INSTALL_LOG);
  assert.deepEqual(signed, [DEPLOY_TX]);
  const skipped = /Skipping install because wasm already installed/.test(SKIPPED_INSTALL_LOG);
  assert.equal(skipped, true);
});

test('fresh-upload deploy yields upload tx then create tx, in order', () => {
  assert.deepEqual(extractSigningHashes(UPLOAD_LOG), [UPLOAD_TX, GOV_DEPLOY_TX]);
  assert.deepEqual(extractExplorerTxHashes(UPLOAD_LOG), [UPLOAD_TX, GOV_DEPLOY_TX]);
});

test('wasm sha256 is never returned as a transaction hash', () => {
  for (const log of [SKIPPED_INSTALL_LOG, UPLOAD_LOG]) {
    assert.ok(!extractSigningHashes(log).includes(WASM_SHA));
    assert.ok(!extractSigningHashes(log).includes(GOV_WASM_SHA));
    assert.ok(!extractExplorerTxHashes(log).includes(WASM_SHA));
  }
});

test('contract id and explorer tx hashes parse from structured lines', () => {
  assert.equal(
    extractContractId(SKIPPED_INSTALL_LOG),
    'CBLEYZZL3LRZO2O4EU6S2JE72H6ZIYREPSDZCD7QXP3V45FDHCKI63JQ',
  );
  assert.deepEqual(extractExplorerTxHashes(SKIPPED_INSTALL_LOG), [DEPLOY_TX]);
});
