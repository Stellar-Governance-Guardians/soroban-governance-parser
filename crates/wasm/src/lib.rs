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
