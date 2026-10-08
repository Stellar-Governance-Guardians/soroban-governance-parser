// Raw-capturing Soroban JSON-RPC client + transaction pipeline.
//
// Process rule 3/4: every RPC response used as evidence is written verbatim to
// .seed/capture/ (gitignored working area); capture-fixtures.js later curates
// the committed fixtures under tests/fixtures/seed-v2/. Nothing is modified
// between the wire and the file.
import { mkdirSync, readdirSync, appendFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { TransactionBuilder, Contract, SorobanDataBuilder, xdr } from '@stellar/stellar-sdk';
import { rpc } from '@stellar/stellar-sdk';
import { CAPTURE_DIR, loadDeployments, assertNetwork } from './config.js';
import { addressOf, keypairOf } from './keys.js';

export const NETWORK = assertNetwork(loadDeployments());
export const PASSPHRASE = NETWORK.networkPassphrase;
export const SERVER = new rpc.Server(NETWORK.rpcUrl, { allowHttp: NETWORK.rpcUrl.startsWith('http:') });

export function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/**
 * Block until `ledger` has CLOSED, i.e. until getHealth reports a strictly
 * greater latestLedger.
 *
 * Why this exists (see docs/seed-v2.md "OpenZeppelin checkpoint-index root
 * cause"): a transaction is visible to getTransaction as soon as it is
 * *included* in a ledger, which happens BEFORE that ledger closes. Returning at
 * inclusion time lets the next operation simulate against a snapshot that does
 * not yet include the previous transaction's state changes. OpenZeppelin's
 * votes storage derives its write key from a counter held in instance storage
 * (`NumTotalSupplyCheckpoints`), so a one-ledger-stale snapshot predicts
 * checkpoint index N while execution needs N+1, and the transaction traps with
 * `error {storage: exceeded_limit}` on a key that was never in the footprint.
 *
 * Waiting for close makes the next simulation observe at least the same state
 * execution will. Fail-closed: throws rather than silently continuing on a
 * snapshot known to be stale.
 */
export async function waitForLedgerClose(ledger, { note = `close-${ledger}`, timeoutMs = 180000 } = {}) {
  if (!Number.isFinite(ledger)) throw new Error('waitForLedgerClose requires a numeric ledger (fail closed)');
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const health = requireOk(await rpcRaw('getHealth', {}, { note, capture: false }), `getHealth(${note})`);
    const latest = Number(health.latestLedger);
    if (Number.isFinite(latest) && latest > ledger) return { closedThrough: latest, waitedFor: ledger };
    await sleep(2000);
  }
  throw new Error(`ledger ${ledger} did not close within ${timeoutMs}ms (fail closed)`);
}

mkdirSync(CAPTURE_DIR, { recursive: true });
let captureSeq = existsSync(CAPTURE_DIR)
  ? readdirSync(CAPTURE_DIR).filter((f) => /^\d{4}-/.test(f)).length
  : 0;

function sanitize(note) {
  return (note || '').replace(/[^a-zA-Z0-9._-]+/g, '_').slice(0, 80);
}

function captureFile(method, params, response, note) {
  captureSeq += 1;
  const file = `${String(captureSeq).padStart(4, '0')}-${sanitize(method)}${note ? `-${sanitize(note)}` : ''}.json`;
  const payload = {
    capturedAtUtc: new Date().toISOString(),
    request: { method, params },
    response,
  };
  writeFileSync(join(CAPTURE_DIR, file), `${JSON.stringify(payload, null, 2)}\n`);
  appendFileSync(join(CAPTURE_DIR, 'index.jsonl'), `${JSON.stringify({ file, method, note: note || '' })}\n`);
  return file;
}

function isTransient(error) {
  const code = error?.code;
  const msg = String(error?.message ?? '');
  return code === -32603 || code === -32602 || /try again later|too early|queue/i.test(msg);
}

/** POST a JSON-RPC method; capture the verbatim response; retry transient failures. */
export async function rpcRaw(method, params, { note = '', retries = 4, capture = true } = {}) {
  const body = { jsonrpc: '2.0', id: (captureSeq + 1) * 1000 + (captureSeq % 997), method, params };
  let lastErr = null;
  for (let attempt = 0; attempt <= retries; attempt += 1) {
    try {
      const res = await fetch(NETWORK.rpcUrl, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      });
      const text = await res.text();
      let json;
      try {
        json = JSON.parse(text);
      } catch {
        throw new Error(`RPC HTTP ${res.status} non-JSON body: ${text.slice(0, 300)}`);
      }
      if (!res.ok) {
        if (attempt < retries) {
          await sleep(1000 * (attempt + 1));
          continue;
        }
        throw new Error(`RPC HTTP ${res.status}: ${text.slice(0, 300)}`);
      }
      if (json.error && isTransient(json.error) && attempt < retries) {
        await sleep(1000 * (attempt + 1));
        continue;
      }
      if (capture) captureFile(method, params, json, note);
      return json;
    } catch (e) {
      lastErr = e;
      if (attempt < retries) await sleep(1000 * (attempt + 1));
    }
  }
  throw lastErr ?? new Error(`rpcRaw(${method}) failed with no error captured`);
}

export async function health(note = 'health') {
  return rpcRaw('getHealth', {}, { note });
}

/** Capture a getEvents window (contract-filtered) verbatim. */
export async function captureEvents({ startLedger, endLedger, contractIds }, note) {
  if (!contractIds?.length) throw new Error('captureEvents requires contractIds (fail closed)');
  return rpcRaw('getEvents', { startLedger, endLedger, filters: [{ type: 'contract', contractIds }] }, { note });
}

/** Capture getLedgerEntries for base64 XDR LedgerEntryKeys verbatim. */
export async function captureLedgerEntries(keys, note) {
  if (!keys?.length) throw new Error('captureLedgerEntries requires keys');
  return rpcRaw('getLedgerEntries', { keys, xdrFormat: 'XDR' }, { note });
}

function requireOk(json, what) {
  if (json.error) throw new Error(`${what}: RPC error ${JSON.stringify(json.error)}`);
  if (!json.result) throw new Error(`${what}: no result in response: ${JSON.stringify(json).slice(0, 400)}`);
  return json.result;
}

// --- resource margin & transaction-result decoding -------------------------
//
// Simulation is a snapshot. At execution the write/instruction footprint can be
// a few bytes larger than simulation declared, and ANY shortfall fails the whole
// transaction with `invoke_host_function: resource_limit_exceeded` (diagnostic
// event `budget: exceeded_limit`, "operation byte-write resources exceeds amount
// specified"). This is the root cause of the observed Script3 S2 failure:
// declared write_bytes 1232, actual 1240. Over-declaring resources is always
// allowed (you only pay a slightly larger resource fee), so we pad the simulated
// footprint before signing. See docs/seed-v2.md for the full diagnosis.
const RESOURCE_MARGIN_RATIO = 0.15;
const RESOURCE_MARGIN_FLAT = 64;

function padded(value) {
  return Math.ceil(Number(value) * (1 + RESOURCE_MARGIN_RATIO)) + RESOURCE_MARGIN_FLAT;
}

/**
 * Rebuild an assembled transaction with the simulated Soroban resource
 * footprint padded by a fixed + proportional margin. Operations (including
 * simulation-provided auth) are preserved via TransactionBuilder.cloneFrom.
 */
export function withResourceMargin(assembledTx, transactionDataB64) {
  if (!transactionDataB64) {
    throw new Error('withResourceMargin requires the simulation transactionData (fail closed)');
  }
  const base = new SorobanDataBuilder(transactionDataB64).build().resources;
  const inflated = new SorobanDataBuilder(transactionDataB64)
    .setResources(padded(base.instructions), padded(base.diskReadBytes), padded(base.writeBytes))
    .build();
  return TransactionBuilder.cloneFrom(assembledTx, {
    fee: assembledTx.fee,
    sorobanData: inflated,
    networkPassphrase: PASSPHRASE,
  }).build();
}

/**
 * Decode a base64 TransactionResult (getTransaction resultXdr) or a failed-send
 * errorResultXdr into JSON. Never throws: an undecodable result becomes
 * `{ decode_error }` so the caller still records *something* auditable.
 */
export function decodeTransactionResult(xdrB64) {
  if (!xdrB64) return null;
  try {
    return xdr.TransactionResult.fromXDR(xdrB64, 'base64').toJSON();
  } catch (e) {
    return { decode_error: e.message };
  }
}

/**
 * Build, simulate (raw capture), assemble, sign, send (raw capture) and wait
 * (raw getTransaction capture) a contract invocation. Throws on simulation
 * error, send error, or any non-SUCCESS terminal status. Failed transactions
 * carry `err.decodedResult` (decoded resultXdr), `err.txHash` and `err.ledger`.
 */
export async function invoke({ contractId, fn, args = [], sourceId, signerId = sourceId, note = fn }) {
  const source = await SERVER.getAccount(addressOf(sourceId));
  const contract = new Contract(contractId);
  const tx = new TransactionBuilder(source, { fee: '100', networkPassphrase: PASSPHRASE })
    .addOperation(contract.call(fn, ...args))
    .setTimeout(180)
    .build();

  const sim = await rpcRaw('simulateTransaction', { transaction: tx.toXDR() }, { note: `simulate-${note}` });
  const simResult = requireOk(sim, `simulate(${note})`);
  if (sim.error || !simResult.results) {
    throw new Error(`simulation failed for ${note}: ${JSON.stringify(sim.error ?? simResult).slice(0, 800)}`);
  }

  const assembled = rpc.assembleTransaction(tx, simResult).build();
  // Pad the simulated resource footprint before signing: see withResourceMargin.
  const prepared = withResourceMargin(assembled, simResult.transactionData);
  // sign() mutates in place and returns void
  prepared.sign(keypairOf(signerId));
  const signed = prepared;

  const send = await rpcRaw('sendTransaction', { transaction: signed.toXDR() }, { note: `send-${note}` });
  const sendResult = requireOk(send, `send(${note})`);
  if (sendResult.status === 'ERROR') {
    const decoded = decodeTransactionResult(sendResult.errorResultXdr);
    const err = new Error(`sendTransaction ERROR for ${note}: ${JSON.stringify(decoded ?? sendResult)}`);
    err.decodedResult = decoded;
    throw err;
  }
  const hash = sendResult.hash;

  let got = null;
  for (let i = 0; i < 60; i += 1) {
    got = await rpcRaw('getTransaction', { hash }, { note: `wait-${note}`, capture: false });
    const status = got?.result?.status ?? 'NOT_FOUND';
    if (status !== 'NOT_FOUND') break;
    await sleep(1500);
  }
  const final = await rpcRaw('getTransaction', { hash }, { note: `final-${note}` });
  const status = final?.result?.status;
  if (status !== 'SUCCESS') {
    const decoded = decodeTransactionResult(final.result.resultXdr);
    const err = new Error(`tx ${hash} (${note}) ended as ${status}: ${JSON.stringify(decoded)}`);
    err.decodedResult = decoded;
    err.txHash = hash;
    err.ledger = final.result.ledger;
    throw err;
  }
  // Do NOT return at inclusion time. getTransaction resolves as soon as the tx
  // is in a ledger, which is before that ledger closes; the next simulate would
  // then run against a snapshot missing this tx's state changes and predict a
  // stale checkpoint index. See waitForLedgerClose.
  const closed = await waitForLedgerClose(final.result.ledger, { note: `close-${note}` });
  return { hash, ledger: final.result.ledger, closedThrough: closed.closedThrough, sim: simResult, status, note };
}

/**
 * Read-only invocation: simulate a contract call without sending it.
 * Captures the raw simulateTransaction response (differential-test evidence)
 * and returns the decoded return value when present.
 */
export async function simulateRead({ contractId, fn, args = [], sourceId, note = `read-${fn}` }) {
  const source = await SERVER.getAccount(addressOf(sourceId));
  const contract = new Contract(contractId);
  const tx = new TransactionBuilder(source, { fee: '100', networkPassphrase: PASSPHRASE })
    .addOperation(contract.call(fn, ...args))
    .setTimeout(60)
    .build();
  const sim = await rpcRaw('simulateTransaction', { transaction: tx.toXDR() }, { note });
  const simResult = requireOk(sim, `simulateRead(${note})`);
  if (sim.error || !simResult.results) {
    throw new Error(`read simulation failed for ${note}: ${JSON.stringify(sim.error ?? simResult).slice(0, 800)}`);
  }
  const retXdr = simResult.results[0]?.xdr ?? null;
  return { raw: simResult, returnXdr: retXdr, note };
}
