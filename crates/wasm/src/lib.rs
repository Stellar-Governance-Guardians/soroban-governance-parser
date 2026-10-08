//! wasm-bindgen surface for the parser core. Thin layer only: all logic and
//! all fail-closed behavior live in `soroban-governance-core`. Errors are
//! surfaced as JS exceptions with explicit messages — never silent defaults.

use soroban_governance_core::{spec, DecodeError};
use stellar_xdr::{Limits, ReadXdr, ScVal};
use wasm_bindgen::prelude::*;

fn err_to_js(e: &DecodeError) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Decode a base64 XDR `ScVal` into a JSON string.
#[wasm_bindgen]
pub fn decode_scval_base64(xdr_base64: &str) -> Result<String, JsValue> {
    let scval = ScVal::from_xdr_base64(xdr_base64, Limits::none())
        .map_err(|e| JsValue::from_str(&format!("input is not valid XDR base64 ScVal: {e}")))?;
    let json = soroban_governance_core::scval::scval_to_json(&scval).map_err(|e| err_to_js(&e))?;
    serde_json::to_string(&json).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse the `contractspecv0` custom section of contract WASM bytes into a
/// JSON string describing functions, structs, enums and errors.
#[wasm_bindgen]
pub fn parse_contract_spec(wasm_bytes: &[u8]) -> Result<String, JsValue> {
    let spec = spec::parse_contract_spec(wasm_bytes).map_err(|e| err_to_js(&e))?;
    let json = spec.to_json().map_err(|e| err_to_js(&e))?;
    serde_json::to_string(&json).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Classify a verified function name against the published risk rule table.
#[wasm_bindgen]
pub fn classify_risk(function_name: &str, verified: bool) -> Result<String, JsValue> {
    let c = if verified {
        soroban_governance_core::risk::classify_verified(function_name)
    } else {
        soroban_governance_core::risk::classify_unverified(function_name)
    };
    serde_json::to_string(&c).map_err(|e| JsValue::from_str(&e.to_string()))
}

// --- Script3 adapter surface (N3: adapter decode + risk + tally exported) ------

fn adapter_err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Reconstruct Script3 proposal state from a captured `get_proposal` return
/// value (base64 XDR). Offline and pure; fails closed with a JS exception.
///
/// Returns the `Script3ProposalState` as JSON.
#[wasm_bindgen]
pub fn script3_state_from_chain(proposal_id: u32, get_proposal_xdr_base64: &str) -> Result<String, JsValue> {
    use stellar_xdr::ReadXdr;
    let scval = ScVal::from_xdr_base64(get_proposal_xdr_base64, Limits::none())
        .map_err(|e| JsValue::from_str(&format!("input is not valid XDR base64 ScVal: {e}")))?;
    let adapter = soroban_governance_core::adapters::script3::Script3Adapter;
    let state = adapter.state_from_chain(proposal_id, &scval).map_err(adapter_err)?;
    serde_json::to_string(&state).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Reconstruct a Script3 tally from a captured `get_proposal_votes` return
/// value (base64 XDR). Offline and pure.
#[wasm_bindgen]
pub fn script3_tally_from_chain(get_proposal_votes_xdr_base64: &str) -> Result<String, JsValue> {
    use stellar_xdr::ReadXdr;
    let scval = ScVal::from_xdr_base64(get_proposal_votes_xdr_base64, Limits::none())
        .map_err(|e| JsValue::from_str(&format!("input is not valid XDR base64 ScVal: {e}")))?;
    let adapter = soroban_governance_core::adapters::script3::Script3Adapter;
    let tally = adapter.tally_from_chain(&scval).map_err(adapter_err)?;
    serde_json::to_string(&tally).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Decode a Script3 proposal state into the shared `DecodedProposal` shape
/// (calls + published risk rules). `risk_context_json` is a serialized
/// `RiskContext` (`{governor, treasury_addresses, large_value_threshold}`);
/// pass `{}` for defaults (no contextual rules fire).
#[wasm_bindgen]
pub fn script3_decode_proposal(state_json: &str, risk_context_json: &str) -> Result<String, JsValue> {
    let state: soroban_governance_core::adapters::script3::Script3ProposalState =
        serde_json::from_str(state_json)
            .map_err(|e| JsValue::from_str(&format!("state_json is not a Script3ProposalState: {e}")))?;
    let context: soroban_governance_core::risk::RiskContext =
        serde_json::from_str(risk_context_json)
            .map_err(|e| JsValue::from_str(&format!("risk_context_json is not a RiskContext: {e}")))?;
    let adapter = soroban_governance_core::adapters::script3::Script3Adapter;
    let proposal = adapter.decode_proposal(&state, &context).map_err(adapter_err)?;
    serde_json::to_string(&proposal).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Evaluate the published contextual risk rules against one call.
/// `call_json` is a serialized `CallUnderReview`.
#[wasm_bindgen]
pub fn evaluate_risk_context(call_json: &str, risk_context_json: &str) -> Result<String, JsValue> {
    use soroban_governance_core::risk::{CallUnderReview, RiskContext};
    let context: RiskContext = serde_json::from_str(risk_context_json)
        .map_err(|e| JsValue::from_str(&format!("risk_context_json is not a RiskContext: {e}")))?;
    let call: CallUnderReview = serde_json::from_str(call_json)
        .map_err(|e| JsValue::from_str(&format!("call_json is not a CallUnderReview: {e}")))?;
    serde_json::to_string(&context.evaluate(&call)).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Evaluate a tally against governor settings: `settings_json` is a
/// `GovernorSettings`, `tally_json` a `VoteCount`. Returns the
/// `VoteOutcome` as a snake_case string (`successful` / `no_quorum` /
/// `threshold_not_met`).
#[wasm_bindgen]
pub fn evaluate_tally(tally_json: &str, settings_json: &str, total_supply: i128) -> Result<String, JsValue> {
    use soroban_governance_core::tally::{evaluate, GovernorSettings, VoteCount};
    let tally: VoteCount = serde_json::from_str(tally_json)
        .map_err(|e| JsValue::from_str(&format!("tally_json is not a VoteCount: {e}")))?;
    let settings: GovernorSettings = serde_json::from_str(settings_json)
        .map_err(|e| JsValue::from_str(&format!("settings_json is not a GovernorSettings: {e}")))?;
    let outcome = evaluate(&tally, &settings, total_supply);
    serde_json::to_string(&outcome).map_err(|e| JsValue::from_str(&e.to_string()))
}
