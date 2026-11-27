//! OFFLINE differential tests for the OpenZeppelin governor adapter.
//!
//! Inputs are committed raw Soroban RPC captures under
//! `tests/fixtures/seed-v2/rpc/`; expectations were read from the same
//! captures independently of the adapter. No network access, so a testnet
//! reset cannot invalidate these tests.
//!
//! Scope, stated honestly: the corpus contains `proposal_state` reads for the
//! three OpenZeppelin proposals that reached a vote window (`o2`, `o4`, `o6`)
//! plus the byte-reproducible governor WASM. It does **not** contain a
//! `getEvents` capture for this governor, so event payload layouts are checked
//! against the governor's embedded contract spec (names) and the pinned
//! upstream source (field layout), not against a captured event.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use soroban_governance_core::adapters::openzeppelin::{
    OpenZeppelinAdapter, OzProposalStatus, ADAPTER_NAME, EVENT_ARITY, GOVERNOR_CONTRACT_ID,
};
use soroban_governance_core::adapters::{governor_for_contract, registry};
use soroban_governance_core::spec;
use stellar_xdr::{Limits, ReadXdr, ScVal};

fn fixtures_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/seed-v2")
        .canonicalize()
        .expect("committed fixtures must exist")
}

/// Newest capture matching a note suffix, decoded from
/// `response.result.results[0].xdr` as an `ScVal`.
fn decoded_return(suffix: &str) -> ScVal {
    let dir = fixtures_root().join("rpc");
    let mut matches: Vec<_> = std::fs::read_dir(&dir)
        .expect("rpc fixture dir")
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with(|c: char| c.is_ascii_digit()) && name.contains(suffix)
        })
        .collect();
    matches.sort_by_key(std::fs::DirEntry::path);
    let newest = matches
        .pop()
        .unwrap_or_else(|| panic!("no committed fixture matching `{suffix}`"));
    let text = std::fs::read_to_string(newest.path()).expect("fixture is readable");
    let json: serde_json::Value = serde_json::from_str(&text).expect("fixture is valid JSON");
    let xdr = json["response"]["result"]["results"][0]["xdr"]
        .as_str()
        .unwrap_or_else(|| panic!("fixture `{suffix}` has no results[0].xdr"));
    ScVal::from_xdr_base64(xdr, Limits::none()).expect("captured ScVal must decode")
}

/// Every captured `proposal_state` read, newest capture per proposal, with the
/// discriminant the capture actually returned. Values were read off the raw
/// XDR (`AAAAAwAAAAE=` is U32(1), `AAAAAwAAAAI=` is U32(2)); `o2`, `o4` and
/// `o6` closed defeated, the rest were still active when captured.
const CAPTURED_STATES: &[(&str, OzProposalStatus)] = &[
    (
        "read-oz-proposal_state-o1-transfer-executed",
        OzProposalStatus::Active,
    ),
    (
        "read-oz-proposal_state-o2-contract-upgrade",
        OzProposalStatus::Defeated,
    ),
    (
        "read-oz-proposal_state-o3-admin-change",
        OzProposalStatus::Active,
    ),
    (
        "read-oz-proposal_state-o4-unknown-contract-call",
        OzProposalStatus::Defeated,
    ),
    (
        "read-oz-proposal_state-o5-failing-quorum",
        OzProposalStatus::Active,
    ),
    (
        "read-oz-proposal_state-o6-abstain-heavy",
        OzProposalStatus::Defeated,
    ),
];

#[test]
fn captured_proposal_state_reads_decode_to_their_recorded_state() {
    let adapter = OpenZeppelinAdapter;
    for (suffix, expected) in CAPTURED_STATES {
        // The capture is keyed by its fixture note, not by a proposal id; the
        // read itself contains no id, so a well-formed placeholder is used.
        let proposal_id = "00".repeat(32);
        let state = adapter
            .state_from_chain(&proposal_id, &decoded_return(suffix))
            .unwrap_or_else(|e| panic!("{suffix} must decode: {e}"));
        assert_eq!(state.governor, ADAPTER_NAME);
        assert_eq!(state.status, *expected, "captured state for {suffix}");
    }
}

#[test]
fn both_adapters_are_registered_without_overlap() {
    let adapters = registry();
    assert_eq!(adapters.len(), 2);
    assert_eq!(adapters[0].name(), "script3-soroban-governor");
    assert_eq!(adapters[1].name(), ADAPTER_NAME);
    assert_eq!(
        governor_for_contract(GOVERNOR_CONTRACT_ID),
        Some(ADAPTER_NAME)
    );
    assert_eq!(
        governor_for_contract("CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX"),
        Some("script3-soroban-governor")
    );
    assert_eq!(governor_for_contract("CUNKNOWN"), None);
}

#[test]
fn deployed_governor_spec_declares_every_event_the_adapter_claims() {
    let wasm = std::fs::read(fixtures_root().join("wasms/oz-fungible-governor-contract.wasm"))
        .expect("committed governor wasm");
    let contract_spec = spec::parse_contract_spec(&wasm).expect("contractspecv0 parses");
    let declared: Vec<String> = contract_spec
        .events
        .iter()
        .map(|name| {
            // The spec names the event struct (`ProposalCreated`); the macro's
            // default topic is the snake_case form the adapter matches on.
            name.chars()
                .enumerate()
                .flat_map(|(i, c)| {
                    if c.is_uppercase() && i > 0 {
                        vec!['_', c.to_ascii_lowercase()]
                    } else {
                        vec![c.to_ascii_lowercase()]
                    }
                })
                .collect()
        })
        .collect();
    // `proposal_queued` is emitted only by governor deployments that enable
    // queuing. This deployment does not, so the SDK's spec shaking removes it
    // from the embedded spec; the adapter still knows the upstream shape so a
    // queuing deployment can be added without a code change.
    let spec_shaken = ["proposal_queued"];
    for (event, arity) in EVENT_ARITY {
        if !spec_shaken.contains(event) {
            assert!(
                declared.iter().any(|name| name == event),
                "the deployed governor's spec must declare `{event}`; declared: {declared:?}"
            );
        }
        assert!(*arity >= 1, "every event carries its name as topic 0");
    }
}
