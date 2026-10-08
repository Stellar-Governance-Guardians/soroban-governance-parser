// Identity management. Keys live in the stellar CLI keystore under the
// gitignored .seed/ directory — never in tracked files (charter rule: no
// secrets). All seed identities are throwaway, friendbot-funded testnet keys.
import { mkdirSync } from 'node:fs';
import { Keypair } from '@stellar/stellar-sdk';
import { stellar } from './cli.js';
import { KEYS_DIR } from './config.js';

const base = ['--config-dir', KEYS_DIR];

export function listIdentities() {
  const { stdout } = stellar(['keys', 'ls', ...base]);
  return stdout.split('\n').map((s) => s.trim()).filter(Boolean);
}

/** Create the identity (friendbot-funded) if missing. Returns its address. */
export function ensureIdentity(name) {
  mkdirSync(KEYS_DIR, { recursive: true });
  if (!listIdentities().includes(name)) {
    stellar(['keys', 'generate', name, '--as-secret', '--network', 'testnet', '--fund', ...base], { logName: `key-${name}` });
  }
  return addressOf(name);
}

export function addressOf(name) {
  return stellar(['keys', 'public-key', name, ...base]).stdout.trim();
}

export function keypairOf(name) {
  const secret = stellar(['keys', 'secret', name, ...base]).stdout.trim();
  return Keypair.fromSecret(secret);
}
