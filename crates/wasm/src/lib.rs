//! wasm-bindgen surface for the parser core. Thin layer only: all logic and
//! all fail-closed behavior live in `soroban-governance-core`. Errors are
//! surfaced as JS exceptions with explicit messages — never silent defaults.

use soroban_governance_core::adapter::{GovernorAdapter, RawCall, RawEvent};
use soroban_governance_core::adapters::script3::Script3Adapter;
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
pub fn script3_state_from_chain(
    proposal_id: u32,
    get_proposal_xdr_base64: &str,
) -> Result<String, JsValue> {
    use stellar_xdr::ReadXdr;
    let scval = ScVal::from_xdr_base64(get_proposal_xdr_base64, Limits::none())
        .map_err(|e| JsValue::from_str(&format!("input is not valid XDR base64 ScVal: {e}")))?;
    let adapter = soroban_governance_core::adapters::script3::Script3Adapter;
    let state = adapter
        .state_from_chain(proposal_id, &scval)
        .map_err(adapter_err)?;
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
pub fn script3_decode_proposal(
    state_json: &str,
    risk_context_json: &str,
) -> Result<String, JsValue> {
    let state: soroban_governance_core::adapters::script3::Script3ProposalState =
        serde_json::from_str(state_json).map_err(|e| {
            JsValue::from_str(&format!("state_json is not a Script3ProposalState: {e}"))
        })?;
    let context: soroban_governance_core::risk::RiskContext =
        serde_json::from_str(risk_context_json).map_err(|e| {
            JsValue::from_str(&format!("risk_context_json is not a RiskContext: {e}"))
        })?;
    let adapter = soroban_governance_core::adapters::script3::Script3Adapter;
    let proposal = adapter
        .decode_proposal(&state, &context)
        .map_err(adapter_err)?;
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
pub fn evaluate_tally(
    tally_json: &str,
    settings_json: &str,
    total_supply: i128,
) -> Result<String, JsValue> {
    use soroban_governance_core::tally::{evaluate, GovernorSettings, VoteCount};
    let tally: VoteCount = serde_json::from_str(tally_json)
        .map_err(|e| JsValue::from_str(&format!("tally_json is not a VoteCount: {e}")))?;
    let settings: GovernorSettings = serde_json::from_str(settings_json)
        .map_err(|e| JsValue::from_str(&format!("settings_json is not a GovernorSettings: {e}")))?;
    let outcome = evaluate(&tally, &settings, total_supply);
    serde_json::to_string(&outcome).map_err(|e| JsValue::from_str(&e.to_string()))
}

// --- Governor adapter exports for browser use ---

/// Identify the governor adapter used by this WASM build.
/// Returns the adapter name (e.g. "script3") so downstream code can
/// reason about which governor is active.
#[wasm_bindgen]
pub fn governor_identify() -> String {
    let adapter = Script3Adapter;
    adapter.name().to_owned()
}

/// Check whether a governance event belongs to this governor.
///
/// This function uses the same logic as `Script3Adapter::matches_event`:
/// - The event must come from the governor contract (CA contract ID)
/// - topic[0] must be a known Script3 event symbol with matching arity
/// - If the symbol or arity doesn't match, returns `false` (never guesses),
///   so the browser pipeline falls back to `unverified` decoding.
///
/// Parameters:
/// - `contract_id`: the governor contract identifier
/// - `ledger`: the ledger number
/// - `topic0_b64`: base64-encoded topic[0] ScVal (the event symbol)
/// - `topic_count`: the number of topics in the event
/// - `value_b64`: base64-encoded value ScVal
#[wasm_bindgen]
pub fn governor_matches_event(
    contract_id: &str,
    ledger: u32,
    topic0_b64: &str,
    _topic_count: u32,
    value_b64: &str,
) -> bool {
    // Decode topic[0] from base64 XDR - must be a Symbol
    let scval_topic0: ScVal = match ScVal::from_xdr_base64(topic0_b64, Limits::none()) {
        Ok(val) => val,
        Err(_) => return false,
    };
    // Decode value from base64 XDR
    let scval_value: ScVal = match ScVal::from_xdr_base64(value_b64, Limits::none()) {
        Ok(val) => val,
        Err(_) => return false,
    };

    // Construct RawEvent with topic[0] as the only topic
    let raw_event = RawEvent {
        contract_id,
        ledger,
        tx_hash: "",
        topics: std::slice::from_ref(&scval_topic0),
        value: &scval_value,
    };

    let adapter = Script3Adapter;
    adapter.matches_event(&raw_event)
}

/// Normalize a matched governance event. Fail-closed: if the payload does not
/// match the verified shape, return an explicit error — never a partial or
/// defaulted decode. This is the same logic as the Script3Adapter::normalize_event
/// but exposed to the browser so pages can render the exact failure reason.
///
/// The event is specified with topic[0] as the event symbol (base64 XDR),
/// the full topic count, and the value (base64 XDR). This matches the logic
/// of Script3Adapter::normalize_event which fail-closures on unexpected payload
/// shapes and returns explicit AdapterError variants.
pub fn governor_normalize_event(
    contract_id: &str,
    ledger: u32,
    topic0_b64: &str,
    _topic_count: u32,
    value_b64: &str,
) -> Result<String, JsValue> {
    // Decode topic[0] from base64 XDR - must be a Symbol
    let scval_topic0: ScVal = match ScVal::from_xdr_base64(topic0_b64, Limits::none()) {
        Ok(val) => val,
        Err(e) => return Err(JsValue::from_str(&format!("invalid topic0: {e}"))),
    };
    // Decode value from base64 XDR
    let scval_value: ScVal = match ScVal::from_xdr_base64(value_b64, Limits::none()) {
        Ok(val) => val,
        Err(e) => return Err(JsValue::from_str(&format!("invalid value: {e}"))),
    };

    // Construct RawEvent with topic[0] as the only topic
    let raw_event = RawEvent {
        contract_id,
        ledger,
        tx_hash: "",
        topics: std::slice::from_ref(&scval_topic0),
        value: &scval_value,
    };

    let adapter = Script3Adapter;
    match adapter.normalize_event(&raw_event) {
        Ok(norm) => serde_json::to_string(&norm).map_err(|e| JsValue::from_str(&e.to_string())),
        Err(e) => Err(JsValue::from_str(&format!("normalization error: {e}"))),
    }
}

/// Decode a proposal's contract call into a `DecodedCall` shape. Without the
/// contract spec, args are returned positional with `decoding: Unverified`
/// (charter rule 2: never invent names). With the spec, named arguments are
/// used when available.
///
/// The call is specified with args as a JSON string (array of base64 XDR ScVal
/// values) and the function name.
pub fn governor_decode_call(
    function_name: &str,
    args_json: &str,
    spec_json: &str,
) -> Result<String, JsValue> {
    // Parse args from JSON array
    let args_value: serde_json::Value = match serde_json::from_str(args_json) {
        Ok(v) => v,
        Err(e) => return Err(JsValue::from_str(&format!("invalid args JSON: {e}"))),
    };
    let scval_args: Vec<ScVal> = match args_value {
        serde_json::Value::Array(arr) => {
            let mut result = Vec::new();
            for v in arr {
                if let Some(s) = v.as_str() {
                    match ScVal::from_xdr_base64(s, Limits::none()) {
                        Ok(scval) => result.push(scval),
                        Err(_) => return Err(JsValue::from_str("invalid arg base64")),
                    }
                } else {
                    return Err(JsValue::from_str("each arg must be a base64 string"));
                }
            }
            result
        }
        _ => return Err(JsValue::from_str("args must be a JSON array")),
    };

    // Parse the contract spec from JSON
    let spec: soroban_governance_core::spec::ContractSpec = match serde_json::from_str(spec_json) {
        Ok(s) => s,
        Err(e) => return Err(JsValue::from_str(&format!("invalid spec: {e}"))),
    };

    // Construct RawCall
    let raw_call = RawCall {
        contract_id: "",
        function_name,
        args: &scval_args,
    };

    let adapter = Script3Adapter;
    match adapter.decode_call(&raw_call, Some(&spec)) {
        Ok(decoded) => {
            serde_json::to_string(&decoded).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        Err(e) => Err(JsValue::from_str(&format!("decode error: {e}"))),
    }
}

// Stable JSON-in/JSON-out adapter surface.  The older convenience functions
// above are retained for compatibility with alpha.1 consumers; these exports
// carry the complete topic vector and transaction hash required by the
// governance-v1 NormalizedEvent shape.

fn decode_scvals_json(json: &str, label: &str) -> Result<Vec<ScVal>, JsValue> {
    let values: Vec<String> = serde_json::from_str(json).map_err(|e| {
        JsValue::from_str(&format!(
            "{label} must be a JSON array of base64 XDR ScVals: {e}"
        ))
    })?;
    values
        .iter()
        .map(|value| {
            ScVal::from_xdr_base64(value, Limits::none())
                .map_err(|e| JsValue::from_str(&format!("{label} contains invalid ScVal XDR: {e}")))
        })
        .collect()
}

#[wasm_bindgen(js_name = identify_governor)]
pub fn identify_governor() -> String {
    governor_identify()
}

#[wasm_bindgen(js_name = matches_event)]
pub fn matches_event(
    contract_id: &str,
    ledger: u32,
    tx_hash: &str,
    topics_json: &str,
    value_b64: &str,
) -> Result<bool, JsValue> {
    let topics = decode_scvals_json(topics_json, "topics")?;
    let value = ScVal::from_xdr_base64(value_b64, Limits::none())
        .map_err(|e| JsValue::from_str(&format!("value is not valid ScVal XDR: {e}")))?;
    let event = RawEvent {
        contract_id,
        ledger,
        tx_hash,
        topics: &topics,
        value: &value,
    };
    Ok(Script3Adapter.matches_event(&event))
}

#[wasm_bindgen(js_name = normalize_event)]
pub fn normalize_event(
    contract_id: &str,
    ledger: u32,
    tx_hash: &str,
    topics_json: &str,
    value_b64: &str,
) -> Result<String, JsValue> {
    let topics = decode_scvals_json(topics_json, "topics")?;
    let value = ScVal::from_xdr_base64(value_b64, Limits::none())
        .map_err(|e| JsValue::from_str(&format!("value is not valid ScVal XDR: {e}")))?;
    let event = RawEvent {
        contract_id,
        ledger,
        tx_hash,
        topics: &topics,
        value: &value,
    };
    let normalized = Script3Adapter
        .normalize_event(&event)
        .map_err(|e| JsValue::from_str(&format!("event normalization failed: {e}")))?;
    serde_json::to_string(&normalized).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[wasm_bindgen(js_name = decode_call)]
#[allow(clippy::needless_pass_by_value)]
pub fn decode_call(
    contract_id: &str,
    function_name: &str,
    args_json: &str,
    spec_json: Option<String>,
) -> Result<String, JsValue> {
    let args = decode_scvals_json(args_json, "args")?;
    let spec = spec_json
        .as_deref()
        .map(serde_json::from_str::<soroban_governance_core::spec::ContractSpec>)
        .transpose()
        .map_err(|e| JsValue::from_str(&format!("spec is not valid ContractSpec JSON: {e}")))?;
    let call = RawCall {
        contract_id,
        function_name,
        args: &args,
    };
    let decoded = Script3Adapter
        .decode_call(&call, spec.as_ref())
        .map_err(|e| JsValue::from_str(&format!("call decoding failed: {e}")))?;
    serde_json::to_string(&decoded).map_err(|e| JsValue::from_str(&e.to_string()))
}
