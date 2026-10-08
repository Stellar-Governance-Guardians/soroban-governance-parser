// The six seed proposals per governor. Six proposals cover all seven required
// shapes (the "passing and executed" proposal is also the token transfer):
//   1 token transfer  -> also the passing + executed proposal (safeToExecute)
//   2 contract upgrade
//   3 admin change
//   4 unknown-contract call
//   5 failing quorum (never voted on)
//   6 abstain-heavy
// Descriptions are fixed strings: OpenZeppelin proposal IDs are
// keccak256 over (actions, keccak256(description)), so descriptions must be
// stable and unique within a governor.
import jsSha3 from 'js-sha3';

const { keccak256 } = jsSha3;
import { scAddress, scString, scSymbol, scU32, scI128, scStruct, scEnum, scVec, scBytes32 } from './scval.js';
import { TRANSFER_AMOUNT, SUBCALL_AMOUNT, E7 } from './config.js';

export { SUPPORT as VOTE_SUPPORT };

export const SUPPORT = { against: 0, for: 1, abstain: 2 };

/** Delegation pairs: real self-signed delegations on both vote tokens. */
export const DELEGATIONS = [
  { from: 'sgg-delegate-4', to: 'sgg-delegate-1' },
  { from: 'sgg-delegate-5', to: 'sgg-delegate-2' },
];

/**
 * Script3 proposals.
 * `action(ctx)` builds the ProposalAction ScVal; ctx = { identities, script3Votes,
 * script3Governor, script3MockSubcall, script3GovernorWasmHash }.
 */
export const SCRIPT3_PROPOSALS = [
  {
    key: 's1-transfer-executed',
    shape: 'token transfer + passing/executed',
    creatorId: 'sgg-delegate-1',
    title: 'Seed v2: treasury transfer to delegate-1',
    description: 'Seed v2 S1: transfer 1000 GOV from the governor treasury to delegate-1. Passes with quorum and executes.',
    safeToExecute: true,
    action: (ctx) => scEnum('Calldata', scStruct({
      contract_id: scAddress(ctx.script3Votes),
      function: scSymbol('transfer'),
      args: scVec([scAddress(ctx.script3Governor), scAddress(ctx.identities['sgg-delegate-1']), scI128(TRANSFER_AMOUNT)]),
      auths: scVec([]),
    })),
    votes: [
      { voterId: 'sgg-delegate-1', support: SUPPORT.for },
      { voterId: 'sgg-delegate-2', support: SUPPORT.for },
      { voterId: 'sgg-delegate-3', support: SUPPORT.for },
    ],
  },
  {
    key: 's2-contract-upgrade',
    shape: 'contract upgrade',
    creatorId: 'sgg-deployer', // Script3: Upgrade proposals may only be created by the council
    title: 'Seed v2: governor code upgrade',
    description: 'Seed v2 S2: upgrade the governor contract to its own current wasm hash (no-op code upgrade; proves upgrade-action decode). Never executed by the activity script.',
    safeToExecute: false,
    action: (ctx) => scEnum('Upgrade', scBytes32(ctx.script3GovernorWasmHash)),
    votes: [{ voterId: 'sgg-delegate-1', support: SUPPORT.for }],
  },
  {
    key: 's3-admin-change',
    shape: 'admin change',
    creatorId: 'sgg-delegate-2',
    title: 'Seed v2: council seat change',
    description: 'Seed v2 S3: replace the governor council with delegate-3.',
    safeToExecute: false,
    action: (ctx) => scEnum('Council', scAddress(ctx.identities['sgg-delegate-3'])),
    votes: [
      { voterId: 'sgg-delegate-2', support: SUPPORT.for },
      { voterId: 'sgg-delegate-3', support: SUPPORT.for },
    ],
  },
  {
    key: 's4-unknown-contract-call',
    shape: 'unknown-contract call',
    creatorId: 'sgg-delegate-3',
    title: 'Seed v2: call outside the governance surface',
    description: 'Seed v2 S4: invoke subcall.no_auth_sc on the mock-subcall contract — a target outside any governance allowlist.',
    safeToExecute: false,
    action: (ctx) => scEnum('Calldata', scStruct({
      contract_id: scAddress(ctx.script3MockSubcall),
      function: scSymbol('no_auth_sc'),
      args: scVec([scI128(SUBCALL_AMOUNT)]),
      auths: scVec([]),
    })),
    votes: [{ voterId: 'sgg-delegate-3', support: SUPPORT.for }],
  },
  {
    key: 's5-failing-quorum',
    shape: 'failing quorum',
    creatorId: 'sgg-delegate-4',
    title: 'Seed v2: quorum failure probe',
    description: 'Seed v2 S5: token transfer nobody votes on — must fail quorum at close.',
    safeToExecute: false,
    action: (ctx) => scEnum('Calldata', scStruct({
      contract_id: scAddress(ctx.script3Votes),
      function: scSymbol('transfer'),
      args: scVec([scAddress(ctx.script3Governor), scAddress(ctx.identities['sgg-delegate-4']), scI128(250n * E7)]),
      auths: scVec([]),
    })),
    votes: [],
  },
  {
    key: 's6-abstain-heavy',
    shape: 'abstain-heavy',
    creatorId: 'sgg-delegate-5',
    title: 'Seed v2: abstain-heavy probe',
    description: 'Seed v2 S6: token transfer voted only with abstains — quorum participation but zero for-votes.',
    safeToExecute: false,
    action: (ctx) => scEnum('Calldata', scStruct({
      contract_id: scAddress(ctx.script3Votes),
      function: scSymbol('transfer'),
      args: scVec([scAddress(ctx.script3Governor), scAddress(ctx.identities['sgg-delegate-5']), scI128(500n * E7)]),
      auths: scVec([]),
    })),
    votes: [
      { voterId: 'sgg-delegate-1', support: SUPPORT.abstain },
      { voterId: 'sgg-delegate-2', support: SUPPORT.abstain },
      { voterId: 'sgg-delegate-3', support: SUPPORT.abstain },
    ],
  },
];

/**
 * OpenZeppelin proposals. `call(ctx)` returns { targets, functions, args,
 * description } as ScVals for propose(), and the same raw values plus the
 * keccak256 description hash are reused by settle.js for execute().
 */
export const OZ_PROPOSALS = [
  {
    key: 'o1-transfer-executed',
    shape: 'token transfer + passing/executed',
    creatorId: 'sgg-delegate-1',
    description: 'Seed v2 O1: transfer 1000 GOV from the governor treasury to delegate-1. Passes with quorum and executes.',
    safeToExecute: true,
    call: (ctx) => ({
      targets: [scAddress(ctx.ozToken)],
      functions: [scSymbol('transfer')],
      args: [scVec([scAddress(ctx.ozGovernor), scAddress(ctx.identities['sgg-delegate-1']), scI128(TRANSFER_AMOUNT)])],
      description: 'Seed v2 O1: transfer 1000 GOV from the governor treasury to delegate-1. Passes with quorum and executes.',
    }),
    votes: [
      { voterId: 'sgg-delegate-1', support: SUPPORT.for },
      { voterId: 'sgg-delegate-2', support: SUPPORT.for },
      { voterId: 'sgg-delegate-3', support: SUPPORT.for },
    ],
  },
  {
    key: 'o2-contract-upgrade',
    shape: 'contract upgrade',
    creatorId: 'sgg-delegate-2',
    description: 'Seed v2 O2: upgrade the example upgradeable contract to its own current wasm hash (proves upgrade-action decode; not executed — upstream role auth binds execution to the admin).',
    safeToExecute: false,
    call: (ctx) => ({
      targets: [scAddress(ctx.ozUpgradeableV1)],
      functions: [scSymbol('upgrade')],
      args: [scVec([scBytes32(ctx.ozUpgradeableV1WasmHash), scAddress(ctx.identities['sgg-deployer'])])],
      description: 'Seed v2 O2: upgrade the example upgradeable contract to its own current wasm hash (proves upgrade-action decode; not executed — upstream role auth binds execution to the admin).',
    }),
    votes: [{ voterId: 'sgg-delegate-1', support: SUPPORT.for }],
  },
  {
    key: 'o3-admin-change',
    shape: 'admin change',
    creatorId: 'sgg-delegate-3',
    description: 'Seed v2 O3: transfer token ownership to delegate-3.',
    safeToExecute: false,
    call: (ctx) => ({
      targets: [scAddress(ctx.ozToken)],
      functions: [scSymbol('transfer_ownership')],
      args: [scVec([scAddress(ctx.identities['sgg-delegate-3']), scU32(8_100_000)])],
      description: 'Seed v2 O3: transfer token ownership to delegate-3.',
    }),
    votes: [
      { voterId: 'sgg-delegate-2', support: SUPPORT.for },
      { voterId: 'sgg-delegate-3', support: SUPPORT.for },
    ],
  },
  {
    key: 'o4-unknown-contract-call',
    shape: 'unknown-contract call',
    creatorId: 'sgg-delegate-4',
    description: 'Seed v2 O4: invoke subcall.no_auth_sc on a contract outside the governance surface.',
    safeToExecute: false,
    call: (ctx) => ({
      targets: [scAddress(ctx.ozMockSubcall)],
      functions: [scSymbol('no_auth_sc')],
      args: [scVec([scI128(SUBCALL_AMOUNT)])],
      description: 'Seed v2 O4: invoke subcall.no_auth_sc on a contract outside the governance surface.',
    }),
    votes: [{ voterId: 'sgg-delegate-3', support: SUPPORT.for }],
  },
  {
    key: 'o5-failing-quorum',
    shape: 'failing quorum',
    creatorId: 'sgg-delegate-5',
    description: 'Seed v2 O5: token transfer nobody votes on — must fail quorum.',
    safeToExecute: false,
    call: (ctx) => ({
      targets: [scAddress(ctx.ozToken)],
      functions: [scSymbol('transfer')],
      args: [scVec([scAddress(ctx.ozGovernor), scAddress(ctx.identities['sgg-delegate-5']), scI128(250n * E7)])],
      description: 'Seed v2 O5: token transfer nobody votes on — must fail quorum.',
    }),
    votes: [],
  },
  {
    key: 'o6-abstain-heavy',
    shape: 'abstain-heavy',
    creatorId: 'sgg-deployer',
    description: 'Seed v2 O6: token transfer voted only with abstains.',
    safeToExecute: false,
    call: (ctx) => ({
      targets: [scAddress(ctx.ozToken)],
      functions: [scSymbol('transfer')],
      args: [scVec([scAddress(ctx.ozGovernor), scAddress(ctx.identities['sgg-delegate-1']), scI128(500n * E7)])],
      description: 'Seed v2 O6: token transfer voted only with abstains.',
    }),
    votes: [
      { voterId: 'sgg-delegate-1', support: SUPPORT.abstain },
      { voterId: 'sgg-delegate-2', support: SUPPORT.abstain },
      { voterId: 'sgg-delegate-3', support: SUPPORT.abstain },
    ],
  },
];

/** OpenZeppelin description hash rule: keccak256(description raw bytes). */
export function ozDescriptionHashHex(description) {
  return keccak256(description);
}
