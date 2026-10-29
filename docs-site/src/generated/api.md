# API reference (`governance-v1`)

Generated from `schemas/governance-v1.graphql` by `docs-site/scripts/gen-api.py`. Do not edit by hand; CI regenerates it and fails on any diff. SDL sha256: `cfbd85a82ee834ac14723cd5dd73ce18a5aba0313a91a0aaf6053d1a486ae275`.

This SDL is the shared interface contract. The indexer serves it; the dashboard validates every response field against it. Bumping it requires a `schema.lock` update in each consumer repo, and the parser's PR gate refuses non-additive changes to the v1 schema.

Server status: **not served yet.** The indexer currently stops at raw event ingestion (I2); the GraphQL server (I4) is planned. Response examples on this page are the schema, not captured responses.

## `schema`

Root operations: `query: Query`.

## `enum` DecodingStatus

| Value | Description |
|---|---|
| `VERIFIED` | Names verified against the contract's contractspecv0 section. |
| `UNVERIFIED` | Raw ScVal passthrough; spec unavailable or governor unrecognized. |

## `enum` RiskTier

| Value | Description |
|---|---|
| `CRITICAL` |  |
| `HIGH` |  |
| `MEDIUM` |  |
| `LOW` |  |
| `UNVERIFIED` |  |

## `type` RiskClassification

| Field | Type | Arguments | Description |
|---|---|---|---|
| `tier` | `RiskTier!` |  |  |
| `matchedRule` | `String!` |  | The exact published rule that matched, or the exact reason for UNVERIFIED. Classifications are data, not verdicts — the rule is always exposed. |
| `verified` | `Boolean!` |  |  |

## `type` DecodedCall

| Field | Type | Arguments | Description |
|---|---|---|---|
| `contractId` | `String!` |  |  |
| `functionName` | `String!` |  |  |
| `arguments` | `JSON!` |  | JSON: named object when decoding=VERIFIED, positional array when UNVERIFIED. |
| `decoding` | `DecodingStatus!` |  |  |
| `risk` | `RiskClassification!` |  |  |

## `type` DecodedProposal

| Field | Type | Arguments | Description |
|---|---|---|---|
| `governor` | `String!` |  | Adapter id, or the literal 'unverified'. Never guessed. |
| `proposalId` | `String!` |  |  |
| `proposer` | `String` |  |  |
| `calls` | `[DecodedCall!]!` |  |  |
| `decoding` | `DecodingStatus!` |  |  |

## `type` NormalizedEvent

| Field | Type | Arguments | Description |
|---|---|---|---|
| `governor` | `String!` |  |  |
| `topics` | `[String!]!` |  |  |
| `ledger` | `Int!` |  |  |
| `txHash` | `String!` |  |  |
| `contractId` | `String!` |  |  |
| `payload` | `JSON!` |  |  |
| `decoding` | `DecodingStatus!` |  |  |

## `type` ExecutionImpact

Dry-run simulation output. ALWAYS an estimate (charter rule 5): it runs against the current ledger, not the future execution ledger. Timelocks and state changes between now and execution can alter the real result.

| Field | Type | Arguments | Description |
|---|---|---|---|
| `estimate` | `Boolean!` |  |  |
| `simulatedAtLedger` | `Int!` |  |  |
| `cpuInstructions` | `String` |  |  |
| `memoryBytes` | `String` |  |  |
| `minResourceFeeStroops` | `String` |  |  |
| `stateChanges` | `JSON` |  | null when the RPC returned no state changes; absence is meaningful. |
| `decodeStatus` | `DecodingStatus!` |  |  |

## `type` DelegateMetrics

Delegate metric: raw counts + published formula. No composite scores.

| Field | Type | Arguments | Description |
|---|---|---|---|
| `delegate` | `String!` |  |  |
| `votesCast` | `Int!` |  | Number of governance votes cast by this delegate that we indexed. |
| `proposalsEligible` | `Int!` |  | Number of proposals that were open while this delegate held voting power. |
| `participationRatio` | `Float` |  | votesCast / proposalsEligible. Inputs are exposed above; formula published in docs. A ratio, not a verdict. |
| `delegatedPower` | `String!` |  |  |

## `scalar` JSON

Opaque scalar serialized as JSON.

## `type` Query

| Field | Type | Arguments | Description |
|---|---|---|---|
| `events` | `EventConnection!` | `contractId`: `String`<br>`fromLedger`: `Int`<br>`toLedger`: `Int`<br>`first`: `Int` = 50<br>`after`: `String` | Governance events indexed from live testnet, oldest first. |
| `proposal` | `DecodedProposal` | `proposalId`: `String!`<br>`governorContract`: `String!` |  |
| `executionImpact` | `ExecutionImpact!` | `proposalId`: `String!`<br>`governorContract`: `String!` | Dry-run impact estimate for a proposal. Every field is labeled estimate. |
| `delegateMetrics` | `DelegateMetrics!` | `delegate`: `String!` |  |
| `indexerHealth` | `IndexerHealth!` |  | Indexer health: cursor position, RPC retention window, lag. Fail-loud: a stale or broken indexer surfaces here, never as an empty result set. |

## `type` IndexerHealth

| Field | Type | Arguments | Description |
|---|---|---|---|
| `cursorLedger` | `Int!` |  | Last ledger the indexer successfully processed. |
| `latestLedger` | `Int!` |  |  |
| `oldestLedger` | `Int!` |  |  |
| `ledgerRetentionWindow` | `Int!` |  | Retention window of the RPC in use, as reported by getHealth. |
| `lagLedgers` | `Int!` |  |  |
| `healthy` | `Boolean!` |  |  |

## `type` EventConnection

| Field | Type | Arguments | Description |
|---|---|---|---|
| `edges` | `[EventEdge!]!` |  |  |
| `pageInfo` | `PageInfo!` |  |  |

## `type` EventEdge

| Field | Type | Arguments | Description |
|---|---|---|---|
| `cursor` | `String!` |  |  |
| `node` | `NormalizedEvent!` |  |  |

## `type` PageInfo

| Field | Type | Arguments | Description |
|---|---|---|---|
| `hasNextPage` | `Boolean!` |  |  |
| `endCursor` | `String` |  |  |

## Full SDL

```graphql
# Soroban Governance — GraphQL SDL v1
# Single source of truth shared by governance-event-indexer (server) and
# delegate-portal-dashboard (client). Field names mirror schema-v1.json.
# Bumping this version requires a schema.lock update in every consumer repo.

schema {
  query: Query
}

enum DecodingStatus {
  """Names verified against the contract's contractspecv0 section."""
  VERIFIED
  """Raw ScVal passthrough; spec unavailable or governor unrecognized."""
  UNVERIFIED
}

enum RiskTier {
  CRITICAL
  HIGH
  MEDIUM
  LOW
  UNVERIFIED
}

type RiskClassification {
  tier: RiskTier!
  """The exact published rule that matched, or the exact reason for UNVERIFIED.
  Classifications are data, not verdicts — the rule is always exposed."""
  matchedRule: String!
  verified: Boolean!
}

type DecodedCall {
  contractId: String!
  functionName: String!
  """JSON: named object when decoding=VERIFIED, positional array when UNVERIFIED."""
  arguments: JSON!
  decoding: DecodingStatus!
  risk: RiskClassification!
}

type DecodedProposal {
  """Adapter id, or the literal 'unverified'. Never guessed."""
  governor: String!
  proposalId: String!
  proposer: String
  calls: [DecodedCall!]!
  decoding: DecodingStatus!
}

type NormalizedEvent {
  governor: String!
  topics: [String!]!
  ledger: Int!
  txHash: String!
  contractId: String!
  payload: JSON!
  decoding: DecodingStatus!
}

"""Dry-run simulation output. ALWAYS an estimate (charter rule 5): it runs
against the current ledger, not the future execution ledger. Timelocks and
state changes between now and execution can alter the real result."""
type ExecutionImpact {
  estimate: Boolean!
  simulatedAtLedger: Int!
  cpuInstructions: String
  memoryBytes: String
  minResourceFeeStroops: String
  """null when the RPC returned no state changes; absence is meaningful."""
  stateChanges: JSON
  decodeStatus: DecodingStatus!
}

"""Delegate metric: raw counts + published formula. No composite scores."""
type DelegateMetrics {
  delegate: String!
  """Number of governance votes cast by this delegate that we indexed."""
  votesCast: Int!
  """Number of proposals that were open while this delegate held voting power."""
  proposalsEligible: Int!
  """votesCast / proposalsEligible. Inputs are exposed above; formula published
  in docs. A ratio, not a verdict."""
  participationRatio: Float
  delegatedPower: String!
}

scalar JSON

type Query {
  """Governance events indexed from live testnet, oldest first."""
  events(
    contractId: String
    fromLedger: Int
    toLedger: Int
    first: Int = 50
    after: String
  ): EventConnection!

  proposal(proposalId: String!, governorContract: String!): DecodedProposal

  """Dry-run impact estimate for a proposal. Every field is labeled estimate."""
  executionImpact(proposalId: String!, governorContract: String!): ExecutionImpact!

  delegateMetrics(delegate: String!): DelegateMetrics!

  """Indexer health: cursor position, RPC retention window, lag. Fail-loud:
  a stale or broken indexer surfaces here, never as an empty result set."""
  indexerHealth: IndexerHealth!
}

type IndexerHealth {
  """Last ledger the indexer successfully processed."""
  cursorLedger: Int!
  latestLedger: Int!
  oldestLedger: Int!
  """Retention window of the RPC in use, as reported by getHealth."""
  ledgerRetentionWindow: Int!
  lagLedgers: Int!
  healthy: Boolean!
}

type EventConnection {
  edges: [EventEdge!]!
  pageInfo: PageInfo!
}

type EventEdge {
  cursor: String!
  node: NormalizedEvent!
}

type PageInfo {
  hasNextPage: Boolean!
  endCursor: String
}
```
