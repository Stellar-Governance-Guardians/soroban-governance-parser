//! Sans-IO dry-run (`simulateTransaction`) modeling.
//!
//! Charter rule 5: simulation output is an ESTIMATE. This module only ever
//! produces an [`ExecutionImpact`] with `estimate: true` and a mandatory
//! `simulated_at_ledger`, and it models **only fields observed in real captured
//! responses**. A field the RPC did not return stays `null` — never a default,
//! never a zero.
//!
//! Modeled against the committed raw seed-v2 RPC captures
//! (`*-simulateTransaction-*.json` in the repo's fixture corpus; provenance in
//! its README), whose `response.result` object carries exactly:
//!
//! ```text
//! latestLedger   -> simulated_at_ledger          (always present)
//! minResourceFee -> min_resource_fee_stroops      (string, present on success)
//! stateChanges   -> state_changes                 (present only when the call writes)
//! results[]      -> presence => the call succeeded (no `error` field)
//! ```
//!
//! `cost.cpuInsns` / `cost.memBytes` were **not** present in any captured
//! response, so `cpu_instructions` / `memory_bytes` are always `null` here until
//! a capture demonstrates the shape (rule: verify first). Rent is deliberately
//! not modeled at all — see `docs/dry-run.md`.
//!
//! There is no IO here: [`build_simulate_request`] builds the JSON-RPC request
//! envelope, and the caller (the CLI's `RpcClient`) owns transport and the
//! assembly of the transaction XDR. The `action` -> transaction step is IO and
//! lives in the CLI.

use serde_json::{json, Value};
use stellar_xdr::{Limits, ReadXdr, TransactionEnvelope};

use crate::types::{DecodingStatus, ExecutionImpact};

/// Errors from building or parsing a simulation.
#[derive(Debug, thiserror::Error)]
pub enum SimulateError {
    #[error("simulateTransaction returned an RPC error: {0}")]
    Rpc(String),
    #[error("simulateTransaction response is missing `{0}`: an estimate must always record it")]
    Missing(&'static str),
    #[error("simulateTransaction input/response has an unexpected shape: {0}")]
    Shape(String),
}

/// Build the JSON-RPC 2.0 request body for `simulateTransaction`.
///
/// `transaction_xdr_base64` is the base64 `TransactionEnvelope` XDR; building it
/// from a decoded `action` is the IO layer's job (the CLI), which is why this
/// function takes the already-assembled envelope rather than the action. The
/// envelope is fully validated, fail-closed: a non-base64 or malformed envelope
/// is an error, never a request the node would reject for a reason we could have
/// caught offline.
pub fn build_simulate_request(
    transaction_xdr_base64: &str,
    request_id: u64,
) -> Result<Value, SimulateError> {
    TransactionEnvelope::from_xdr_base64(transaction_xdr_base64, Limits::none()).map_err(|e| {
        SimulateError::Shape(format!(
            "transaction is not a valid TransactionEnvelope XDR: {e}"
        ))
    })?;
    Ok(json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "simulateTransaction",
        "params": { "transaction": transaction_xdr_base64 }
    }))
}

/// Parse a `simulateTransaction` response into an [`ExecutionImpact`].
///
/// Accepts either the full JSON-RPC envelope (`{"result": …}` / `{"error": …}`)
/// or a bare `result` object (as committed in some fixtures). `estimate` is
/// always `true`; `simulated_at_ledger` comes from `latestLedger` and is
/// required — a response without it is an error, never a silent default.
pub fn parse_simulate_response(response: &Value) -> Result<ExecutionImpact, SimulateError> {
    if let Some(err) = response.get("error").filter(|e| !e.is_null()) {
        return Err(SimulateError::Rpc(err.to_string()));
    }
    let result = response.get("result").unwrap_or(response);

    let latest = result
        .get("latestLedger")
        .and_then(Value::as_u64)
        .ok_or(SimulateError::Missing("latestLedger"))?;
    let simulated_at_ledger = u32::try_from(latest)
        .map_err(|_| SimulateError::Shape(format!("latestLedger {latest} exceeds u32")))?;

    let min_resource_fee_stroops = result
        .get("minResourceFee")
        .and_then(Value::as_str)
        .map(str::to_owned);

    // Not observed in any capture; stays null until one demonstrates the shape.
    let (cpu_instructions, memory_bytes) = match result.get("cost") {
        Some(cost) if !cost.is_null() => (
            cost.get("cpuInsns")
                .and_then(Value::as_str)
                .map(str::to_owned),
            cost.get("memBytes")
                .and_then(Value::as_str)
                .map(str::to_owned),
        ),
        _ => (None, None),
    };

    let state_changes = match result.get("stateChanges") {
        Some(Value::Array(a)) if !a.is_empty() => Some(Value::Array(a.clone())),
        _ => None,
    };

    // A successful simulation returns `results`; their XDR is not decoded here,
    // so the impact is explicitly unverified rather than presented as decoded.
    if result.get("results").is_none() {
        return Err(SimulateError::Missing("results"));
    }

    Ok(ExecutionImpact {
        estimate: true,
        simulated_at_ledger,
        cpu_instructions,
        memory_bytes,
        min_resource_fee_stroops,
        state_changes,
        decode_status: DecodingStatus::Unverified,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn minimal_response_models_only_observed_fields() {
        let impact = parse_simulate_response(&json!({
            "result": { "latestLedger": 42, "results": [{"xdr": "AAAAAwAAAAA="}] }
        }))
        .expect("parses");
        assert!(impact.estimate, "estimate must always be true");
        assert_eq!(impact.simulated_at_ledger, 42);
        // Absent fields are null, never defaulted.
        assert_eq!(impact.cpu_instructions, None);
        assert_eq!(impact.memory_bytes, None);
        assert_eq!(impact.min_resource_fee_stroops, None);
        assert_eq!(impact.state_changes, None);
    }

    #[test]
    fn missing_latest_ledger_fails_closed() {
        assert!(matches!(
            parse_simulate_response(&json!({ "result": { "results": [] } })),
            Err(SimulateError::Missing("latestLedger"))
        ));
    }

    #[test]
    fn rpc_error_fails_closed() {
        assert!(matches!(
            parse_simulate_response(&json!({ "error": { "code": -32000, "message": "boom" } })),
            Err(SimulateError::Rpc(_))
        ));
    }

    #[test]
    fn request_builder_rejects_a_non_envelope() {
        assert!(build_simulate_request("not-xdr", 1).is_err());
        assert!(build_simulate_request("", 1).is_err());
    }
}
