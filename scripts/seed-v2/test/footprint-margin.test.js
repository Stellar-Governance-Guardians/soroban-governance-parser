import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { SorobanDataBuilder } from '@stellar/stellar-sdk';
import { widenCheckpointFootprint, CHECKPOINT_INDEX_MARGIN } from '../lib/rpc.js';

const FIX = new URL('../../../tests/fixtures/seed-v2/rpc/', import.meta.url);

// The captured OpenZeppelin mint simulation for delegate-1, which trapped on
// [TotalSupplyCheckpoint, 1] while the declared footprint held only [0].
const TSC_MINT_SIM = '0109-simulateTransaction-simulate-oz-mint-sgg-delegate-1.json';
// A captured Script3 simulation, which declares no checkpoint counter at all.
const SCRIPT3_SIM = '0052-simulateTransaction-simulate-s3-propose-s5-failing-quorum.json';

function simTxData(file) {
  return JSON.parse(readFileSync(new URL(file, FIX), 'utf8')).response.result.transactionData;
}

function checkpointIndices(transactionData) {
  return new SorobanDataBuilder(transactionData)
    .getReadWrite()
    .map((k) => {
      const key = k.value?.key;
      if (key?.type !== 'scvVec' || !Array.isArray(key.vec) || key.vec.length !== 2) return null;
      if (key.vec[0]?.type !== 'scvSymbol' || String(key.vec[0].sym) !== 'TotalSupplyCheckpoint') return null;
      return key.vec[1].type === 'scvU32' ? key.vec[1].u32 : null;
    })
    .filter((i) => i !== null);
}

function isCheckpointKey(arm) {
  const key = arm?.value?.key;
  if (key?.type !== 'scvVec' || !Array.isArray(key.vec) || key.vec.length !== 2) return false;
  return key.vec[0]?.type === 'scvSymbol' && String(key.vec[0].sym) === 'TotalSupplyCheckpoint';
}

function widen(td, margin) {
  return widenCheckpointFootprint(td, margin);
}

test('fixture precondition: the trapping mint declared only checkpoint index 0', () => {
  assert.deepEqual(checkpointIndices(simTxData(TSC_MINT_SIM)), [0]);
});

test('widens the checkpoint-index footprint by the default margin', () => {
  assert.equal(CHECKPOINT_INDEX_MARGIN, 2);
  assert.deepEqual(checkpointIndices(widen(simTxData(TSC_MINT_SIM))), [0, 1, 2]);
});

test('the widening covers the index execution actually needed', () => {
  // delegate-1 trapped wanting [TotalSupplyCheckpoint, 1]; the widened
  // footprint must contain it, which is the whole point of the margin.
  assert.ok(checkpointIndices(widen(simTxData(TSC_MINT_SIM))).includes(1));
});

test('margin is configurable and 0 is a no-op', () => {
  const td = simTxData(TSC_MINT_SIM);
  assert.deepEqual(checkpointIndices(widen(td, 1)), [0, 1]);
  assert.deepEqual(checkpointIndices(widen(td, 0)), [0]);
});

test('is a no-op for a governor with no checkpoint counter (Script3)', () => {
  const td = simTxData(SCRIPT3_SIM);
  assert.deepEqual(checkpointIndices(td), []);
  assert.deepEqual(checkpointIndices(widen(td)), []);
});

test('preserves the pre-existing read-only keys', () => {
  const td = simTxData(TSC_MINT_SIM);
  const before = new SorobanDataBuilder(td).getReadOnly().map((k) => k.toXDR('base64'));
  const after = new SorobanDataBuilder(widen(td)).getReadOnly().map((k) => k.toXDR('base64'));
  assert.deepEqual(after, before);
});

test('preserves the non-checkpoint read-write keys', () => {
  const td = simTxData(TSC_MINT_SIM);
  const strip = (data) =>
    new SorobanDataBuilder(data)
      .getReadWrite()
      .filter((k) => !isCheckpointKey(k))
      .map((k) => k.toXDR('base64'))
      .sort();
  assert.deepEqual(strip(widen(td)), strip(td));
});

test('adds keys only: it never drops or rewrites the simulated keys', () => {
  const td = simTxData(TSC_MINT_SIM);
  const before = new SorobanDataBuilder(td).getReadWrite().map((k) => k.toXDR('base64'));
  const after = new SorobanDataBuilder(widen(td)).getReadWrite().map((k) => k.toXDR('base64'));
  assert.equal(after.length, before.length + CHECKPOINT_INDEX_MARGIN);
  for (const k of before) assert.ok(after.includes(k), `original key ${k} was dropped`);
});

test('leaves the resource values untouched (footprint widening is free)', () => {
  const td = simTxData(TSC_MINT_SIM);
  const before = new SorobanDataBuilder(td).build().resources;
  const after = new SorobanDataBuilder(widen(td)).build().resources;
  assert.equal(after.instructions, before.instructions);
  assert.equal(after.diskReadBytes, before.diskReadBytes);
  assert.equal(after.writeBytes, before.writeBytes);
});

test('declares no duplicate checkpoint indices', () => {
  for (const margin of [1, 2, 3, 5]) {
    const got = checkpointIndices(widen(simTxData(TSC_MINT_SIM), margin));
    assert.deepEqual(got, [...new Set(got)], `duplicate indices at margin ${margin}`);
    assert.deepEqual(got, [...got].sort((a, b) => a - b), `indices not ordered at margin ${margin}`);
  }
});

test('the window is anchored to the highest declared index', () => {
  const td = simTxData(TSC_MINT_SIM);
  const highest = Math.max(...checkpointIndices(td));
  assert.deepEqual(checkpointIndices(widen(td, 2)), [highest, highest + 1, highest + 2]);
});

test('fail-closed on missing simulation data and bad margin', () => {
  const td = simTxData(TSC_MINT_SIM);
  assert.throws(() => widenCheckpointFootprint(''), /fail closed/);
  assert.throws(() => widenCheckpointFootprint(td, -1), /fail closed/);
  assert.throws(() => widenCheckpointFootprint(td, 1.5), /fail closed/);
});
