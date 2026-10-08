// Thin wrapper around the `stellar` CLI. Every failure is loud: non-zero
// exits throw with full output (charter: fail closed and loud).
import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { CAPTURE_DIR } from './config.js';

const CLI_LOG_DIR = join(CAPTURE_DIR, 'cli');

/** Run `stellar <args...>` and return { status, stdout, stderr, out }. */
export function stellar(args, { allowFail = false, logName } = {}) {
  const res = spawnSync('stellar', args, { encoding: 'utf8' });
  const out = `${res.stdout ?? ''}${res.stderr ?? ''}`;
  if (logName) {
    mkdirSync(CLI_LOG_DIR, { recursive: true });
    writeFileSync(join(CLI_LOG_DIR, `${logName}.log`), `$ stellar ${args.join(' ')}\n\n${out}`);
  }
  if (res.error) throw new Error(`failed to run stellar: ${res.error.message}`);
  if (res.status !== 0 && !allowFail) {
    throw new Error(`stellar ${args.join(' ')} failed (exit ${res.status}):\n${out}`);
  }
  return { status: res.status, stdout: res.stdout ?? '', stderr: res.stderr ?? '', out };
}

/** Pull every 64-hex string out of CLI output, in order of appearance. */
export function extractHashes(out) {
  return [...out.matchAll(/\b[0-9a-f]{64}\b/g)].map((m) => m[0]);
}

/** Pull the first Stellar contract id (C...) out of CLI output. */
export function extractContractId(out) {
  const m = out.match(/\b(C[A-Z2-7]{55})\b/);
  return m ? m[0] : null;
}
