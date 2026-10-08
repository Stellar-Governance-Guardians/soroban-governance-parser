// ScVal builders for @stellar/stellar-sdk v17 (new ScVal model: objects with
// .type + value, not legacy js-xdr `.switch()`).
//
// Encodings are verified against the soroban-sdk derive sources that generated
// the deployed contracts (soroban-sdk-macros 20.5.0 / 28.0.0):
//   * `#[contracttype] struct`  -> ScMap with SYMBOL keys, sorted lexicographically
//     (derive_struct.rs: `ScMap::sorted_from(...)`).
//   * `#[contracttype] enum`    -> ScVec whose first element is the variant name
//     as ScSymbol, followed by the variant payload (derive_enum.rs).
// Every constructed value is validated by real simulation before it is ever
// sent (see lib/rpc.js invoke()), so a wrong shape fails closed loudly.
import { nativeToScVal, Address, xdr } from '@stellar/stellar-sdk';

export function scAddress(addr) {
  return new Address(addr).toScVal();
}

export function scString(s) {
  return nativeToScVal(s); // default type is string
}

export function scSymbol(s) {
  return nativeToScVal(s, { type: 'symbol' });
}

export function scU32(n) {
  return nativeToScVal(Number(n), { type: 'u32' });
}

export function scI128(v) {
  return nativeToScVal(BigInt(v), { type: 'i128' });
}

export function scU128(v) {
  return nativeToScVal(BigInt(v), { type: 'u128' });
}

export function scBool(b) {
  return nativeToScVal(Boolean(b));
}

/** BytesN<32> from a hex string. */
export function scBytes32(hex) {
  if (!/^[0-9a-f]{64}$/i.test(hex)) throw new Error(`scBytes32: not 64 hex chars: ${hex}`);
  const bytes = Uint8Array.from(Buffer.from(hex, 'hex'));
  return nativeToScVal(bytes, { type: 'bytes' });
}

/** contracttype struct: { field: ScVal, ... } -> key-sorted ScMap. */
export function scStruct(fields) {
  const entries = Object.keys(fields)
    .sort()
    .map((k) => new xdr.ScMapEntry({ key: scSymbol(k), val: fields[k] }));
  return xdr.ScVal.scvMap(entries);
}

/** contracttype enum variant: scEnum('Calldata', payloadScVal). */
export function scEnum(variant, ...payload) {
  return xdr.ScVal.scvVec([scSymbol(variant), ...payload]);
}

export function scVec(items) {
  return xdr.ScVal.scvVec(items);
}

/** Parse a base64 XDR ScVal (e.g. simulateTransaction return values). */
export function scValFromBase64(b64) {
  return xdr.ScVal.fromXDR(b64, 'base64');
}
