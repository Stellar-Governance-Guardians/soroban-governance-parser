// activity.js — bounded activity generator for the live tier / 72h soak.
//
// Keeps a seeded governor "alive" with real, idempotent on-chain actions so the
// downstream indexer/dashboard always has fresh data: it casts a vote on the
// newest still-open proposal from an eligible voter that has not voted yet, and
// (with --propose) creates one no-op upgrade proposal when a creator is free.
// At most one vote (plus at most one proposal) per run, so it is safe to
// schedule frequently.
//
// Requires the gitignored .seed/ working area (keys + state). Network IO only.
//
// Usage: node activity.js [--once] [--propose]
import { loadState } from './../seed-v2/lib/state.js';
import { DEPLOYER_ID, SCHEDULE } from './../seed-v2/lib/config.js';
import { invoke, simulateRead, health } from './../seed-v2/lib/rpc.js';
import { scAddress, scU32, scString, scEnum, scBytes32 } from './../seed-v2/lib/scval.js';
import { retU32, retSymbolName } from './../seed-v2/lib/parse.js';

const PROPOSE = process.argv.includes('--propose');
const VOTERS = ['sgg-delegate-1', 'sgg-delegate-2', 'sgg-delegate-3'];

/** Cast one vote on the newest open proposal from an un-voted eligible voter. */
async function tryVote(state, latest) {
  const governor = state.contracts.script3Governor.contractId;
  const entries = Object.entries(state.script3?.proposals ?? {}).sort(
    (a, b) => b[1].id - a[1].id,
  );
  for (const [key, rec] of entries) {
    if (latest > rec.createLedger + SCHEDULE.script3.votePeriod) continue; // window closed
    for (const voter of VOTERS) {
      const hv = await simulateRead({
        contractId: governor, fn: 'get_vote',
        args: [scAddress(state.identities[voter]), scU32(rec.id)],
        sourceId: DEPLOYER_ID, note: `activity-get_vote-${key}-${voter}`,
      });
      if (retSymbolName(hv.raw) !== null) continue; // already voted (Some)
      const r = await invoke({
        contractId: governor, fn: 'vote', sourceId: voter,
        args: [scAddress(state.identities[voter]), scU32(rec.id), scU32(1)],
        note: `activity-vote-${key}-${voter}`,
      });
      console.log(`activity: ${voter} voted support=1 on ${key} (tx ${r.hash})`);
      return true;
    }
  }
  return false;
}

/** Create one no-op upgrade proposal from the council deployer, or skip. */
async function tryPropose(state) {
  const governor = state.contracts.script3Governor.contractId;
  const wasmHash = state.contracts.script3Governor.wasmSha256;
  try {
    const r = await invoke({
      contractId: governor, fn: 'propose', sourceId: DEPLOYER_ID,
      args: [
        scAddress(state.identities[DEPLOYER_ID]),
        scString(`Activity probe ${new Date().toISOString()}`),
        scString('Scheduled activity probe (no-op code upgrade; never executed).'),
        scEnum('Upgrade', scBytes32(wasmHash)),
      ],
      note: 'activity-propose',
    });
    console.log(`activity: proposed probe -> id ${retU32(r.sim)} (tx ${r.hash})`);
    return true;
  } catch (e) {
    if (/#211|already open/.test(e.message ?? '')) {
      console.log('activity: creator already has an open proposal — skipping propose');
      return false;
    }
    throw e;
  }
}

async function main() {
  const state = loadState();
  if (!state) throw new Error('no .seed/state.json — run seed.js first (fail closed)');
  const h = await health('activity');
  const latest = h.result.latestLedger;

  let acted = false;
  if (PROPOSE) acted = (await tryPropose(state)) || acted;
  acted = (await tryVote(state, latest)) || acted;
  if (!acted) console.log('activity: nothing to do (no open proposal / all voted)');
}

main().catch((e) => {
  console.error(`ACTIVITY FAILED: ${e.stack ?? e.message}`);
  process.exit(1);
});
