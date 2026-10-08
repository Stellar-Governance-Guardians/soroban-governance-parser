// Parsing helpers for simulation return values (fail-closed: unexpected
// shapes throw rather than guess). Uses the SDK's scValToNative for value
// extraction so shapes are interpreted exactly as the SDK models them.
import { scValToNative } from '@stellar/stellar-sdk';
import { scValFromBase64 } from './scval.js';

/** Return value of the first simulated invocation, as an ScVal (v17 model). */
export function simReturnScVal(simResult) {
  const b64 = simResult?.results?.[0]?.xdr;
  if (!b64) return null;
  return scValFromBase64(b64);
}

function requireType(sv, expected, what) {
  if (!sv) throw new Error(`simulation returned no value (expected ${expected} for ${what})`);
  if (sv.type !== expected) throw new Error(`expected ${expected} return for ${what}, got ${sv.type}`);
  return sv;
}

/** u32 return (Script3 propose). */
export function retU32(simResult, what = 'u32') {
  const sv = requireType(simReturnScVal(simResult), 'scvU32', what);
  return Number(sv.value);
}

/** BytesN<32> return (OpenZeppelin propose) as lowercase hex. */
export function retBytes32Hex(simResult, what = 'bytes32') {
  const sv = requireType(simReturnScVal(simResult), 'scvBytes', what);
  const bytes = sv.value instanceof Uint8Array ? sv.value : sv.value?.value;
  if (!bytes || bytes.length !== 32) throw new Error(`expected 32 bytes for ${what}, got ${bytes?.length}`);
  return Buffer.from(bytes).toString('hex');
}

/**
 * Unit-variant enum return (e.g. OpenZeppelin ProposalState). The encoding is
 * either a bare symbol or a single-element vec wrapping the symbol; accept
 * both, reject everything else (fail closed).
 */
export function retSymbolName(simResult, what = 'enum') {
  const sv = simReturnScVal(simResult);
  if (!sv) throw new Error(`simulation returned no value (expected symbol enum for ${what})`);
  if (sv.type === 'scvVoid') return null; // Option::None
  if (sv.type === 'scvSymbol') return String(scValToNative(sv));
  if (sv.type === 'scvVec') {
    const arr = sv.value ?? [];
    if (arr.length === 1 && arr[0].type === 'scvSymbol') return String(scValToNative(arr[0]));
  }
  throw new Error(`expected symbol enum return for ${what}, got ${sv.type}`);
}

/** Integer return (i128/u128/u32/…) as bigint. */
export function retInt(simResult, what = 'int') {
  const sv = simReturnScVal(simResult);
  if (!sv) throw new Error(`simulation returned no value (expected int for ${what})`);
  if (sv.type === 'scvVoid') throw new Error(`got void instead of int for ${what}`);
  const native = scValToNative(sv);
  if (typeof native === 'bigint') return native;
  if (typeof native === 'number') return BigInt(native);
  throw new Error(`expected int return for ${what}, got ${sv.type} -> ${typeof native}`);
}

/** Native JS view of any return value (annotations only; never for decisions). */
export function retNative(simResult) {
  const sv = simReturnScVal(simResult);
  if (!sv) return null;
  return scValToNative(sv);
}
