// snapshot-power.js — capture power-at-snapshot reads (live testnet).
//
// N3 differential exit criterion: for every Script3 proposal, the parser's
// tally must equal the captured on-chain reads INCLUDING power-at-snapshot.
// The governor derives each vote's weight from
// `get_past_votes(voter, vote_start)` (pinned source, contracts/governor/
// src/contract.rs), but until now the corpus only captured `get_votes`
// (current power). This script fills that gap: it simulates
// `get_past_votes(voter, vote_start)` for every (proposal, voter) pair that
// voted, plus `get_past_total_supply(vote_start)` for the quorum denominator,
// capturing each raw response through the SAME rpcRaw capture path as
// seed.js/settle.js.
//
// Read-only: simulateTransaction never submits. Keys are not needed beyond
// the deployer's public address (simulation source). Idempotent: re-running
// appends fresh captures with new sequence numbers; capture-fixtures.js
// curates the newest set.
//
// Usage: node snapshot-power.js
import { loadState, saveState } from './lib/state.js';
import { DEPLOYER_ID, SCHEDULE } from './lib/config.js';
import { simulateRead, health } from './lib/rpc.js';
import { scAddress, scU32 } from './lib/scval.js';
import { SCRIPT3_PROPOSALS } from './lib/proposals.js';

const S3_VOTE_PERIOD = SCHEDULE.script3.votePeriod;

async function main() {
  const state = loadState();
  if (!state?.contracts?.script3Votes) {
    throw new Error('no script3Votes in .seed/state.json — run deploy.js + seed.js first');
  }
  const votesContract = state.contracts.script3Votes.contractId;
  const proposals = state.script3?.proposals ?? {};

  await health('snapshot-power-start');

  let captured = 0;
  for (const def of SCRIPT3_PROPOSALS) {
    const rec = proposals[def.key];
    if (!rec) throw new Error(`missing Script3 proposal ${def.key} in state (fail closed)`);
    const voteStart = rec.createLedger + SCHEDULE.script3.voteDelay;
    // vote_start is fixed by the proposal record (createLedger + vote_delay);
    // fail closed if it disagrees with the on-chain read we already captured.
    if (rec.reads?.proposalReturnXdr == null) {
      throw new Error(`${def.key}: missing captured get_proposal read (fail closed)`);
    }

    // Quorum denominator at the snapshot ledger.
    await simulateRead({
      contractId: votesContract,
      fn: 'get_past_total_supply',
      sourceId: DEPLOYER_ID,
      args: [scU32(voteStart)],
      note: `read-s3-get_past_total_supply-${def.key}`,
    });
    captured += 1;

    // Per-voter power at the snapshot ledger — this is the weight the
    // governor credits to each cast vote.
    for (const v of def.votes ?? []) {
      const voterAddr = state.identities?.[v.voterId];
      if (!voterAddr) throw new Error(`unknown voter identity ${v.voterId} (fail closed)`);
      await simulateRead({
        contractId: votesContract,
        fn: 'get_past_votes',
        sourceId: DEPLOYER_ID,
        args: [scAddress(voterAddr), scU32(voteStart)],
        note: `read-s3-get_past_votes-${def.key}-${v.voterId}`,
      });
      captured += 1;
    }

    // The creator's power at the snapshot proves the proposal threshold too.
    const creatorAddr = state.identities?.[rec.creatorId];
    if (creatorAddr) {
      await simulateRead({
        contractId: votesContract,
        fn: 'get_past_votes',
        sourceId: DEPLOYER_ID,
        args: [scAddress(creatorAddr), scU32(voteStart)],
        note: `read-s3-get_past_votes-${def.key}-creator-${rec.creatorId}`,
      });
      captured += 1;
    }
  }

  state.updatedAtUtc = new Date().toISOString();
  saveState(state);
  console.log(`snapshot-power: captured ${captured} power-at-snapshot reads`);
}

main().catch((e) => {
  console.error(`SNAPSHOT-POWER FAILED: ${e.message}`);
  process.exit(1);
});
