//! Shared normalized output types. These mirror `schemas/schema.json`
//! (single source of truth lives in schemas/; keep both in sync — CI checks).

use serde::{Deserialize, Serialize};

use crate::risk::RiskClassification;

/// Provenance of a decoding. Anything not backed by a verified contract spec
/// or captured chain data is marked accordingly — never silently assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodingStatus {
    /// Function names and argument names verified via contractspecv0.
    Verified,
    /// Raw ScVal tree only; spec unavailable or governor unrecognized.
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedCall {
    pub contract_id: String,
    pub function_name: String,
    /// Named arguments when verified; positional JSON array when not.
    pub arguments: serde_json::Value,
    pub decoding: DecodingStatus,
    pub risk: RiskClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedProposal {
    pub governor: String,
    pub proposal_id: String,
    pub proposer: Option<String>,
    pub calls: Vec<DecodedCall>,
    pub decoding: DecodingStatus,
}

/// One governance event, normalized across governor implementations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEvent {
    pub governor: String,
    /// Verbatim topic symbols as emitted by the contract.
    pub topics: Vec<String>,
    pub ledger: u32,
    pub tx_hash: String,
    pub contract_id: String,
    pub payload: serde_json::Value,
    pub decoding: DecodingStatus,
}

/// Simulation output labeling — rule 5: every impact value is an ESTIMATE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionImpact {
    /// Always true in v1: dry-run results are estimates, not guarantees.
    pub estimate: bool,
    /// Ledger the simulation ran against.
    pub simulated_at_ledger: u32,
    pub cpu_instructions: Option<String>,
    pub memory_bytes: Option<String>,
    pub min_resource_fee_stroops: Option<String>,
    /// Present only when the RPC actually returned state changes; absence is
    /// meaningful and must not be defaulted to an empty success.
    pub state_changes: Option<serde_json::Value>,
    pub decode_status: DecodingStatus,
}
