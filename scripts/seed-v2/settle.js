// settle.js — matured-proposal settlement, idempotent.
//
// Script3: close() is permissionless but only legal once ledger > vote_end
// (vote_end = createLedger + vote_period 720). The passing transfer proposal
// is then executed (timelock 0 => eta = close ledger).
// OpenZeppelin: state is derived on read; once past vote_end the executable
// proposal can be executed directly (open execution, executor auth).
//
// Every due step is verified; steps not yet due are reported, not failed.
// Re-run at T+~15 min (OZ) and T+~65 min (Script3) after seed.js.
//
// Usage: node settle.js
import { xdr, scValToNative } from '@stellar/stellar-sdk';
import { loadState, saveState } from './lib/state.js';
import { DEPLOYER_ID, SCHEDULE } from './lib/config.js';
import { invoke, simulateRead, health } from './lib/rpc.js';
import { scU32, scBytes32, scAddress, scVec, scValFromBase64 } from './lib/scval.js';
import { retSymbolName } from './lib/parse.js';
import { SCRIPT3_PROPOSALS, OZ_PROPOSALS } from './lib/proposals.js';

const S3_VOTE_PERIOD = SCHEDULE.script3.votePeriod;      // 720
const OZ_VOTING_PERIOD = SCHEDULE.openzeppelin.votingPeriod; // 120

function scvalFromXdrB64(b64) {
  return xdr.ScVal.fromXDR(b64, 'base64');
}

/**
 * Parse Option<Proposal> from get_proposal into a ProposalStatus name or null
 * (None). The native view is: null | [proposalMap] | proposalMap, where the
 * status field is either a bare symbol or a single-element vec. Anything
 * else throws (fail closed — never guess a status).
 */
function parseScript3ProposalStatus(returnXdrB64) {
  if (!returnXdrB64) throw new Error('missing get_proposal return XDR (fail closed)');
  const native = scValToNative(scValFromBase64(returnXdrB64));
  if (native === null || native === undefined) return null; // Option::None
  const describe = () => JSON.stringify(native).slice(0, 300);
  let v = native;
  while (Array.isArray(v) && v.length === 1) v = v[0];
  if (typeof v === 'string') return v; // enum as bare symbol
  if (Array.isArray(v) && typeof v[0] === 'string') return v[0]; // enum as [symbol]
  if (typeof v === 'object' && v.data && typeof v.data === 'object') {
    let st = v.data.status;
    if (Array.isArray(st) && st.length === 1) st = st[0];
    if (typeof st === 'string') return st;
    throw new Error(`get_proposal status is not a symbol: ${JSON.stringify(st)}`);
  }
  throw new Error(`unexpected get_proposal native shape: ${describe()}`);
}

async function settleScript3(state, latestLedger) {
  const governor = state.contracts.script3Governor.contractId;
  const proposals = state.script3?.proposals ?? {};
  console.log('== Script3 settlement ==');
  for (const def of SCRIPT3_PROPOSALS) {
    const rec = proposals[def.key];
    if (!rec) throw new Error(`missing Script3 proposal ${def.key} in state (fail closed)`);
    const voteEnd = rec.createLedger + S3_VOTE_PERIOD;
    rec.voteEnd = voteEnd;

    if (!rec.closed) {
      if (latestLedger <= voteEnd) {
        console.log(`  pending ${def.key}: vote_end ${voteEnd}, current ${latestLedger} (due in ~${(((voteEnd + 1 - latestLedger) * 5) / 60).toFixed(1)} min)`);
        continue;
      }
      const r = await invoke({
        contractId: governor, fn: 'close', sourceId: DEPLOYER_ID,
        args: [scU32(rec.id)], note: `s3-close-${def.key}`,
      });
      rec.closed = true;
      rec.closeTxHash = r.hash;
      rec.closeLedger = r.ledger;
      saveState(state);
      console.log(`  closed ${def.key} (tx ${r.hash.slice(0, 16)}…)`);
    }

    // Capture post-close on-chain reads (differential evidence).
    const getProposal = await simulateRead({
      contractId: governor, fn: 'get_proposal', sourceId: DEPLOYER_ID,
      args: [scU32(rec.id)], note: `read-s3-get_proposal-settled-${def.key}`,
    });
    const status = parseScript3ProposalStatus(getProposal.returnXdr);
    rec.status = status;
    rec.reads ??= {};
    rec.reads.settledProposalReturnXdr = getProposal.returnXdr;
    const getVotes = await simulateRead({
      contractId: governor, fn: 'get_proposal_votes', sourceId: DEPLOYER_ID,
      args: [scU32(rec.id)], note: `read-s3-get_proposal_votes-settled-${def.key}`,
    });
    rec.reads.settledVotesReturnXdr = getVotes.returnXdr;
    saveState(state);
    console.log(`  ${def.key} status=${status ?? 'None'}`);

    if (status === 'Successful' && def.safeToExecute && !rec.executed) {
      const r = await invoke({
        contractId: governor, fn: 'execute', sourceId: DEPLOYER_ID,
        args: [scU32(rec.id)], note: `s3-execute-${def.key}`,
      });
      rec.executed = true;
      rec.executeTxHash = r.hash;
      rec.executeLedger = r.ledger;
      saveState(state);
      console.log(`  EXECUTED ${def.key} (tx ${r.hash.slice(0, 16)}…, ledger ${r.ledger})`);
      const after = await simulateRead({
        contractId: governor, fn: 'get_proposal', sourceId: DEPLOYER_ID,
        args: [scU32(rec.id)], note: `read-s3-get_proposal-executed-${def.key}`,
      });
      rec.status = parseScript3ProposalStatus(after.returnXdr);
      rec.reads.executedProposalReturnXdr = after.returnXdr;
      saveState(state);
      console.log(`  post-execute status=${rec.status}`);
    }
  }
}

async function settleOz(state, latestLedger) {
  const governor = state.contracts.ozGovernor.contractId;
  const proposals = state.oz?.proposals ?? {};
  console.log('== OpenZeppelin settlement ==');
  for (const def of OZ_PROPOSALS) {
    const rec = proposals[def.key];
    if (!rec) throw new Error(`missing OZ proposal ${def.key} in state (fail closed)`);
    const voteEnd = rec.createLedger + OZ_VOTING_PERIOD;
    rec.voteEnd = voteEnd;
    rec.reads ??= {};

    if (latestLedger <= voteEnd) {
      console.log(`  pending ${def.key}: vote_end ${voteEnd}, current ${latestLedger} (due in ~${(((voteEnd + 1 - latestLedger) * 5) / 60).toFixed(1)} min)`);
      continue;
    }

    const stateRead = await simulateRead({
      contractId: governor, fn: 'proposal_state', sourceId: DEPLOYER_ID,
      args: [scBytes32(rec.id)], note: `read-oz-proposal_state-settled-${def.key}`,
    });
    const status = retSymbolName(stateRead);
    rec.status = status;
    rec.reads.settledStateReturnXdr = stateRead.returnXdr;
    saveState(state);
    console.log(`  ${def.key} state=${status}`);

    if (status === 'Succeeded' && def.safeToExecute && !rec.executed) {
      const r = await invoke({
        contractId: governor, fn: 'execute', sourceId: DEPLOYER_ID,
        args: [
          scVec(rec.targets.map((b) => scvalFromXdrB64(b))),
          scVec(rec.functions.map((b) => scvalFromXdrB64(b))),
          scVec(rec.args.map((b) => scvalFromXdrB64(b))),
          scBytes32(rec.descriptionHash),
          scAddress(state.identities[DEPLOYER_ID]),
        ],
        note: `oz-execute-${def.key}`,
      });
      rec.executed = true;
      rec.executeTxHash = r.hash;
      rec.executeLedger = r.ledger;
      saveState(state);
      console.log(`  EXECUTED ${def.key} (tx ${r.hash.slice(0, 16)}…, ledger ${r.ledger})`);
      const after = await simulateRead({
        contractId: governor, fn: 'proposal_state', sourceId: DEPLOYER_ID,
        args: [scBytes32(rec.id)], note: `read-oz-proposal_state-executed-${def.key}`,
      });
      rec.status = retSymbolName(after);
      rec.reads.executedStateReturnXdr = after.returnXdr;
      saveState(state);
      console.log(`  post-execute state=${rec.status}`);
    }
  }
}

async function main() {
  console.log('== seed v2: settle ==');
  const state = loadState();
  if (!state) throw new Error('no .seed/state.json — run seed.js first');
  const h = await health('settle');
  const latestLedger = h.result.latestLedger;
  console.log(`  latest ledger ${latestLedger}`);

  await settleOz(state, latestLedger);
  await settleScript3(state, latestLedger);

  const pendingS3 = SCRIPT3_PROPOSALS.filter((d) => !state.script3.proposals[d.key]?.closed).map((d) => d.key);
  const pendingOz = OZ_PROPOSALS.filter((d) => !state.oz.proposals[d.key]?.status).map((d) => d.key);
  if (pendingS3.length === 0 && pendingOz.length === 0) {
    console.log('SETTLE OK — all proposals settled');
  } else {
    console.log(`SETTLE PARTIAL — not yet due: ${[...pendingS3, ...pendingOz].join(', ')}; re-run after their vote_end ledgers pass`);
  }
}

main().catch((e) => {
  console.error(`SETTLE FAILED: ${e.stack ?? e.message}`);
  process.exit(1);
});
