// seed.js — initialize both governors, mint real voting power, cast real
// delegations, create six proposals each and cast real votes.
//
// Every step is an ordinary signed transaction on live testnet: simulated
// first (raw capture), then sent, then verified SUCCESS before the next step
// runs. Idempotent: completed steps are recorded in .seed/state.json and
// skipped on re-run (re-runnable after a testnet reset by deleting .seed/).
//
// Usage: node seed.js
import jsSha3 from 'js-sha3';
import { loadState, saveState } from './lib/state.js';
import { DEPLOYER_ID, MINTS, SCHEDULE, DECIMALS } from './lib/config.js';
import { invoke, simulateRead, health } from './lib/rpc.js';
import { scAddress, scString, scU32, scI128, scStruct, scVec, scBytes32 } from './lib/scval.js';
import { retU32, retBytes32Hex } from './lib/parse.js';
import { SCRIPT3_PROPOSALS, OZ_PROPOSALS, DELEGATIONS } from './lib/proposals.js';

const { keccak256 } = jsSha3;

function ctxFrom(state) {
  const c = state.contracts;
  const required = ['script3Votes', 'script3Governor', 'script3MockSubcall', 'ozToken', 'ozGovernor', 'ozUpgradeableV1', 'ozMockSubcall'];
  for (const name of required) {
    if (!c[name]?.contractId) throw new Error(`missing contract ${name} — run deploy.js first (fail closed)`);
  }
  if (!state.identities?.[DEPLOYER_ID]) throw new Error('missing identities — run deploy.js first');
  return {
    identities: state.identities,
    script3Votes: c.script3Votes.contractId,
    script3Governor: c.script3Governor.contractId,
    script3MockSubcall: c.script3MockSubcall.contractId,
    script3GovernorWasmHash: c.script3Governor.wasmSha256,
    ozToken: c.ozToken.contractId,
    ozGovernor: c.ozGovernor.contractId,
    ozMockSubcall: c.ozMockSubcall.contractId,
    ozUpgradeableV1: c.ozUpgradeableV1.contractId,
    ozUpgradeableV1WasmHash: c.ozUpgradeableV1.wasmSha256,
  };
}

// Resumability policy (see docs/seed-v2.md):
//  - a step recorded in state.steps is skipped on re-run;
//  - a failed step is recorded under state.failures (status/error/decoded
//    result) and retried automatically on the next run, so the runner never
//    wedges permanently on one step;
//  - SEED_SKIP="<step>,<step>" skips steps explicitly (recorded with a reason);
//  - SEED_KEEP_GOING=1 records a failure and continues to later independent
//    steps instead of aborting the whole run.
const SKIPPED = new Set(
  (process.env.SEED_SKIP ?? '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean),
);
const KEEP_GOING = process.env.SEED_KEEP_GOING === '1';

async function step(state, name, fn) {
  if (state.steps.includes(name)) {
    console.log(`  skip ${name} (already done)`);
    return null;
  }
  if (SKIPPED.has(name)) {
    state.skipped ??= {};
    state.skipped[name] = {
      at: new Date().toISOString(),
      reason: process.env.SEED_SKIP_REASON ?? 'explicit SEED_SKIP',
    };
    saveState(state);
    console.log(`  SKIP ${name} (explicit: ${state.skipped[name].reason})`);
    return null;
  }
  try {
    const result = await fn();
    state.steps.push(name);
    if (state.failures) delete state.failures[name];
    saveState(state);
    return result;
  } catch (e) {
    state.failures ??= {};
    state.failures[name] = {
      at: new Date().toISOString(),
      error: e?.message ?? String(e),
      decodedResult: e?.decodedResult ?? null,
      txHash: e?.txHash ?? null,
      ledger: e?.ledger ?? null,
    };
    saveState(state);
    console.error(`  FAIL ${name}: ${state.failures[name].error}`);
    if (KEEP_GOING) return null;
    throw e;
  }
}

function mintTo(state, tokenContract, notePrefix) {
  // Mint voting power to every identity in MINTS; governor treasury keys map
  // to the governor's own address per flavor.
  return async (recipientKey, amount) => {
    const to = recipientKey === 'sgg-governor-treasury'
      ? tokenContract.governorAddress
      : state.identities[recipientKey];
    if (!to) throw new Error(`no address for mint recipient ${recipientKey}`);
    await step(state, `${notePrefix}-mint-${recipientKey}`, () =>
      invoke({
        contractId: tokenContract.contractId, fn: 'mint', sourceId: DEPLOYER_ID,
        args: [scAddress(to), scI128(amount)],
        note: `${notePrefix}-mint-${recipientKey}`,
      }).then((r) => console.log(`  minted ${amount} -> ${recipientKey.slice(0, 16)}… (tx ${r.hash.slice(0, 16)}…)`)));
  };
}

async function delegateStep(state, tokenContractId, notePrefix) {
  for (const d of DELEGATIONS) {
    await step(state, `${notePrefix}-delegate-${d.from}`, () =>
      invoke({
        contractId: tokenContractId, fn: 'delegate', sourceId: d.from,
        args: [scAddress(state.identities[d.from]), scAddress(state.identities[d.to])],
        note: `${notePrefix}-delegate-${d.from}`,
      }).then((r) => console.log(`  ${d.from} delegated to ${d.to} (tx ${r.hash.slice(0, 16)}…)`)));
  }
}

/**
 * Decide whether a proposal's vote phase can still run. Returns 'ok', or
 * 'skip' when the proposal was never created (KEEP_GOING) or its voting window
 * has already closed at `latest` (a resume after a long gap cannot cast votes
 * that were due in the past). Records the skip reason; never silent.
 */
function votePhase(state, def, rec, latest, votePeriod, scope) {
  if (!rec) {
    if (KEEP_GOING) {
      console.log(`  skip votes for ${def.key}: proposal was not created`);
      return 'skip';
    }
    throw new Error(`proposal ${def.key} missing from state (fail closed)`);
  }
  const voteEnd = rec.createLedger + votePeriod;
  if (latest > voteEnd) {
    state.skipped ??= {};
    state.skipped[`${scope}-votes-${def.key}`] = {
      at: new Date().toISOString(),
      reason: `vote window closed (vote_end ${voteEnd} < latest ${latest})`,
    };
    saveState(state);
    console.log(`  SKIP votes for ${def.key}: window closed (vote_end ${voteEnd}, latest ${latest})`);
    return 'skip';
  }
  return 'ok';
}

async function script3Seed(state, ctx, latest) {
  console.log('== Script3 governor ==');
  const governor = state.contracts.script3Governor.contractId;
  const votes = state.contracts.script3Votes.contractId;

  await step(state, 's3-initialize-votes', async () => {
    const r = await invoke({
      contractId: votes, fn: 'initialize', sourceId: DEPLOYER_ID,
      args: [
        scAddress(state.identities[DEPLOYER_ID]),
        scAddress(governor),
        scU32(DECIMALS),
        scString('Script3 Seed Votes'),
        scString('S3SV'),
      ],
      note: 's3-initialize-votes',
    });
    console.log(`  votes initialized (tx ${r.hash.slice(0, 16)}…)`);
  });

  await step(state, 's3-initialize-governor', async () => {
    const r = await invoke({
      contractId: governor, fn: 'initialize', sourceId: DEPLOYER_ID,
      args: [
        scAddress(votes),
        scAddress(state.identities[DEPLOYER_ID]),
        scStruct({
          counting_type: scU32(SCHEDULE.script3.countingType),
          grace_period: scU32(SCHEDULE.script3.gracePeriod),
          proposal_threshold: scI128(SCHEDULE.script3.proposalThreshold),
          quorum: scU32(SCHEDULE.script3.quorumBps),
          timelock: scU32(SCHEDULE.script3.timelock),
          vote_delay: scU32(SCHEDULE.script3.voteDelay),
          vote_period: scU32(SCHEDULE.script3.votePeriod),
          vote_threshold: scU32(SCHEDULE.script3.voteThresholdBps),
        }),
      ],
      note: 's3-initialize-governor',
    });
    console.log(`  governor initialized (tx ${r.hash.slice(0, 16)}…)`);
  });

  await step(state, 's3-initialize-mock-subcall', async () => {
    const r = await invoke({
      contractId: ctx.script3MockSubcall, fn: 'initialize', sourceId: DEPLOYER_ID,
      args: [scAddress(votes), scAddress(governor)],
      note: 's3-initialize-mock-subcall',
    });
    console.log(`  mock-subcall initialized (tx ${r.hash.slice(0, 16)}…)`);
  });

  console.log('== Script3 mints ==');
  const mintS3 = mintTo(state, { contractId: votes, governorAddress: governor }, 's3');
  for (const [who, amount] of Object.entries(MINTS)) await mintS3(who, amount);

  console.log('== Script3 delegations ==');
  await delegateStep(state, votes, 's3');

  console.log('== Script3 proposals ==');
  for (const def of SCRIPT3_PROPOSALS) {
    await step(state, `s3-propose-${def.key}`, async () => {
      const r = await invoke({
        contractId: governor, fn: 'propose', sourceId: def.creatorId,
        args: [
          scAddress(state.identities[def.creatorId]),
          scString(def.title),
          scString(def.description),
          def.action(ctx),
        ],
        note: `s3-propose-${def.key}`,
      });
      const id = retU32(r.sim);
      state.script3.proposals[def.key] = {
        key: def.key, shape: def.shape, id, idType: 'u32',
        creatorId: def.creatorId, title: def.title, description: def.description,
        safeToExecute: def.safeToExecute,
        createTxHash: r.hash, createLedger: r.ledger,
        closed: false, executed: false, votes: [],
      };
      saveState(state);
      console.log(`  ${def.key} -> id ${id} (tx ${r.hash.slice(0, 16)}…, ledger ${r.ledger})`);
    });
  }

  console.log('== Script3 votes ==');
  for (const def of SCRIPT3_PROPOSALS) {
    const rec = state.script3.proposals[def.key];
    if (votePhase(state, def, rec, latest, SCHEDULE.script3.votePeriod, 's3') === 'skip') continue;
    for (const v of def.votes) {
      await step(state, `s3-vote-${def.key}-${v.voterId}`, async () => {
        const r = await invoke({
          contractId: governor, fn: 'vote', sourceId: v.voterId,
          args: [scAddress(state.identities[v.voterId]), scU32(rec.id), scU32(v.support)],
          note: `s3-vote-${def.key}-${v.voterId}`,
        });
        rec.votes.push({ voterId: v.voterId, support: v.support, txHash: r.hash, ledger: r.ledger });
        saveState(state);
        console.log(`  ${v.voterId} voted support=${v.support} on ${def.key} (tx ${r.hash.slice(0, 16)}…)`);
      });
    }
  }

  console.log('== Script3 on-chain reads (differential evidence) ==');
  for (const def of SCRIPT3_PROPOSALS) {
    const rec = state.script3.proposals[def.key];
    if (!rec) {
      if (KEEP_GOING) continue;
      throw new Error(`proposal ${def.key} missing from state (fail closed)`);
    }
    const idSc = scU32(rec.id);
    rec.reads = rec.reads ?? {};
    const getProposal = await simulateRead({ contractId: governor, fn: 'get_proposal', sourceId: DEPLOYER_ID, args: [idSc], note: `read-s3-get_proposal-${def.key}` });
    rec.reads.proposalReturnXdr = getProposal.returnXdr;
    const getVotes = await simulateRead({ contractId: governor, fn: 'get_proposal_votes', sourceId: DEPLOYER_ID, args: [idSc], note: `read-s3-get_proposal_votes-${def.key}` });
    rec.reads.votesReturnXdr = getVotes.returnXdr;
    saveState(state);
    console.log(`  read ${def.key}: proposal + tallies captured`);
  }
}

async function ozSeed(state, ctx, latest) {
  console.log('== OpenZeppelin governor ==');
  const token = state.contracts.ozToken.contractId;
  const governor = state.contracts.ozGovernor.contractId;

  console.log('== OZ mints ==');
  const mintOZ = mintTo(state, { contractId: token, governorAddress: governor }, 'oz');
  for (const [who, amount] of Object.entries(MINTS)) await mintOZ(who, amount);

  console.log('== OZ delegations ==');
  await delegateStep(state, token, 'oz');

  console.log('== OZ proposals ==');
  for (const def of OZ_PROPOSALS) {
    await step(state, `oz-propose-${def.key}`, async () => {
      const call = def.call(ctx);
      const r = await invoke({
        contractId: governor, fn: 'propose', sourceId: def.creatorId,
        args: [
          scVec(call.targets),
          scVec(call.functions),
          scVec(call.args),
          scString(call.description),
          scAddress(state.identities[def.creatorId]),
        ],
        note: `oz-propose-${def.key}`,
      });
      const id = retBytes32Hex(r.sim);
      // keccak256 of the description's raw bytes (upstream rule)
      const descriptionHash = keccak256(call.description);
      state.oz.proposals[def.key] = {
        key: def.key, shape: def.shape, id, idType: 'hex32',
        creatorId: def.creatorId, description: call.description, descriptionHash,
        safeToExecute: def.safeToExecute,
        targets: call.targets.map((t) => t.toXDR('base64')),
        functions: call.functions.map((f) => f.toXDR('base64')),
        args: call.args.map((a) => a.toXDR('base64')),
        createTxHash: r.hash, createLedger: r.ledger, executed: false, votes: [],
      };
      saveState(state);
      console.log(`  ${def.key} -> id ${id.slice(0, 16)}… (tx ${r.hash.slice(0, 16)}…, ledger ${r.ledger})`);
    });
  }

  console.log('== OZ votes ==');
  for (const def of OZ_PROPOSALS) {
    const rec = state.oz.proposals[def.key];
    if (votePhase(state, def, rec, latest, SCHEDULE.openzeppelin.votingPeriod, 'oz') === 'skip') continue;
    for (const v of def.votes) {
      await step(state, `oz-vote-${def.key}-${v.voterId}`, async () => {
        const r = await invoke({
          contractId: governor, fn: 'cast_vote', sourceId: v.voterId,
          args: [
            scAddress(Buffer.from(rec.id, 'hex')),
            scU32(v.support),
            scString(`Seed v2 vote on ${def.key}`),
            scAddress(state.identities[v.voterId]),
          ],
          note: `oz-vote-${def.key}-${v.voterId}`,
        });
        rec.votes.push({ voterId: v.voterId, support: v.support, txHash: r.hash, ledger: r.ledger });
        saveState(state);
        console.log(`  ${v.voterId} voted support=${v.support} on ${def.key} (tx ${r.hash.slice(0, 16)}…)`);
      });
    }
  }

  console.log('== OZ on-chain reads (differential evidence) ==');
  for (const def of OZ_PROPOSALS) {
    const rec = state.oz.proposals[def.key];
    if (!rec) {
      if (KEEP_GOING) continue;
      throw new Error(`proposal ${def.key} missing from state (fail closed)`);
    }
    rec.reads = rec.reads ?? {};
    const stateRead = await simulateRead({
      contractId: governor, fn: 'proposal_state',
      sourceId: DEPLOYER_ID,
      args: [scBytes32(rec.id)],
      note: `read-oz-proposal_state-${def.key}`,
    });
    rec.reads.stateReturnXdr = stateRead.returnXdr;
    saveState(state);
    console.log(`  read ${def.key}: proposal_state captured`);
  }
}

async function main() {
  console.log('== seed v2: seed ==');
  const state = loadState();
  if (!state) throw new Error('no .seed/state.json — run deploy.js first');
  const h = await health('seed-start');
  const latest = h.result.latestLedger;
  console.log(`  latest ledger ${latest}`);
  const ctx = ctxFrom(state);
  state.script3 ??= { proposals: {} };
  state.oz ??= { proposals: {} };
  saveState(state);

  await script3Seed(state, ctx, latest);
  await ozSeed(state, ctx, latest);

  const failed = Object.keys(state.failures ?? {});
  if (failed.length > 0) {
    console.log(`SEED PARTIAL — failed steps: ${failed.join(', ')}; re-run to retry`);
  } else {
    console.log('SEED OK — all steps recorded in .seed/state.json');
  }
}

main().catch((e) => {
  console.error(`SEED FAILED: ${e.stack ?? e.message}`);
  process.exit(1);
});
