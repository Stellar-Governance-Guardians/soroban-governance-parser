//! OFFLINE dry-run tests against committed `simulateTransaction` captures.
//!
//! Every input is a verbatim raw Soroban RPC response from live testnet under
//! `tests/fixtures/seed-v2/rpc/` (provenance in `tests/fixtures/seed-v2/README.md`
//! and `index.json`). Nothing here touches the network, so a testnet reset cannot
//! change the result.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use soroban_governance_core::simulate::{build_simulate_request, parse_simulate_response};
use soroban_governance_core::types::DecodingStatus;

fn fixture(name: &str) -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/seed-v2/rpc")
        .join(name);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture is JSON")
}

#[test]
fn propose_simulation_models_the_observed_fields() {
    // scripts/seed-v2 captured this when proposing s1 on live testnet.
    let v = fixture("0040-simulateTransaction-simulate-s3-propose-s1-transfer-executed.json");
    let impact = parse_simulate_response(&v["response"]).expect("parses");

    assert!(impact.estimate, "estimate is always true");
    assert_eq!(impact.simulated_at_ledger, 5_083_632, "latestLedger");
    assert_eq!(impact.min_resource_fee_stroops.as_deref(), Some("2937374"));
    // `cost` is not present in this capture, so these must stay null.
    assert_eq!(impact.cpu_instructions, None);
    assert_eq!(impact.memory_bytes, None);
    // The propose call writes, so stateChanges is present and non-empty.
    let changes = impact.state_changes.expect("stateChanges present");
    assert_eq!(changes.as_array().map(Vec::len), Some(6));
    assert_eq!(impact.decode_status, DecodingStatus::Unverified);
}

#[test]
fn read_only_simulation_has_no_state_changes() {
    // A pure read (get_past_votes): no stateChanges, still an estimate.
    let v = fixture(
        "0300-simulateTransaction-read-s3-get_past_votes-s1-transfer-executed-sgg-delegate-1.json",
    );
    let impact = parse_simulate_response(&v["response"]).expect("parses");
    assert!(impact.estimate);
    assert_eq!(impact.simulated_at_ledger, 5_086_168);
    assert_eq!(impact.min_resource_fee_stroops.as_deref(), Some("14240"));
    assert_eq!(impact.state_changes, None, "a read writes nothing");
    assert_eq!(impact.cpu_instructions, None);
    assert_eq!(impact.memory_bytes, None);
}

#[test]
fn build_simulate_request_round_trips_a_real_envelope() {
    // The captured request's transaction XDR is a real TransactionEnvelope.
    let v = fixture("0040-simulateTransaction-simulate-s3-propose-s1-transfer-executed.json");
    let tx = v["request"]["params"]["transaction"]
        .as_str()
        .expect("tx xdr");
    let req = build_simulate_request(tx, 40039).expect("valid envelope");
    assert_eq!(req["jsonrpc"], "2.0");
    assert_eq!(req["method"], "simulateTransaction");
    assert_eq!(req["id"], 40039);
    assert_eq!(req["params"]["transaction"], tx);
}

#[test]
fn build_simulate_request_rejects_garbage() {
    assert!(build_simulate_request("AAAA this is not xdr", 1).is_err());
    assert!(build_simulate_request("", 1).is_err());
}
