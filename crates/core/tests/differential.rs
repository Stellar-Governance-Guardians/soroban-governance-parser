//! OFFLINE differential tests: the parser against captured chain reads.
//!
//! Charter rule 3 in practice. Every expectation in this file was derived
//! *independently* of the parser — by decoding the committed raw RPC captures
//! with `stellar xdr decode` (the reference CLI, not our code) and reading the
//! values by eye — and is then asserted against what `Script3Adapter` produces.
//! A disagreement fails CI.
//!
//! Provenance for every input is `tests/fixtures/seed-v2/index.json`: each
//! fixture is a verbatim Soroban RPC response captured from live testnet, with
//! its method, note, tx hash, status and ledger recorded. Nothing here reaches
//! the network, and nothing depends on the contracts still existing on testnet —
//! a testnet reset cannot make these tests fail.
//!
//! Coverage is every Script3 proposal seeded in the corpus (ids 0-5).

// This is a test crate, not production code: `expect`/`panic` are how a test
// fails loudly (charter: fail closed and loud), and the ledger numbers below
// are atomic values read off chain rather than arithmetic, so digit separators
// would be noise. Mirrors the `#[allow(clippy::expect_used, clippy::unwrap_used)]`
// precedent on every `#[cfg(test)]` module in src/.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::unreadable_literal
)]

use soroban_governance_core::adapters::script3::{
    Script3Action, Script3Adapter, VotePowerModel, GOVERNOR_CONTRACT_ID,
};
use soroban_governance_core::risk::{CallUnderReview, RiskContext, RiskTier};
use soroban_governance_core::tally::{
    evaluate, ProposalStatus, VoteCount, VoteOutcome, BPS_SCALAR,
};
use soroban_governance_core::types::DecodingStatus;
use stellar_xdr::{Limits, ReadXdr, ScVal};

// --- fixture access -----------------------------------------------------------

fn fixture_dir() -> std::path::PathBuf {
    // crates/core/tests/ -> repo root
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/seed-v2")
        .canonicalize()
        .expect("committed fixtures must exist")
}

/// Read `response.result.results[0].xdr` from a fixture and decode it as an ScVal.
///
/// Picks the NEWEST capture for a given note suffix: the corpus contains an
/// earlier aborted run and a later complete run, and only the newest reflects
/// the settled on-chain state.
fn newest_fixture(suffix: &str) -> serde_json::Value {
    let dir = fixture_dir().join("rpc");
    let mut matches: Vec<_> = std::fs::read_dir(&dir)
        .expect("rpc fixture dir")
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with(|c: char| c.is_ascii_digit()) && name.contains(suffix)
        })
        .collect();
    matches.sort_by_key(std::fs::DirEntry::path);
    let last = matches
        .pop()
        .unwrap_or_else(|| panic!("no committed fixture matching `{suffix}`"));
    let text = std::fs::read_to_string(last.path()).expect("fixture is readable");
    let v: serde_json::Value = serde_json::from_str(&text).expect("fixture is valid JSON");
    v
}

fn decoded_return(suffix: &str) -> ScVal {
    let v = newest_fixture(suffix);
    let xdr = v["response"]["result"]["results"][0]["xdr"]
        .as_str()
        .unwrap_or_else(|| panic!("fixture `{suffix}` has no results[0].xdr"));
    ScVal::from_xdr_base64(xdr, Limits::none()).expect("captured ScVal must decode")
}

// --- ground truth, derived from the captures by hand ---------------------------
//
// Live values read from the seeded governor CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX
// and votes contract CCAUJK6V6GIYQANKCV2JDCMCLGMQD42JCF426TQHHMSXW2F5ZROM4IKY.
const TOTAL_SUPPLY: i128 = 15_500_000_000_000;
const QUORUM_BPS: u32 = 100;
const VOTE_THRESHOLD_BPS: u32 = 5000;
const COUNTING_TYPE: u32 = 0b111;
const SETTINGS: soroban_governance_core::tally::GovernorSettings =
    soroban_governance_core::tally::GovernorSettings {
        quorum_bps: QUORUM_BPS,
        vote_threshold_bps: VOTE_THRESHOLD_BPS,
        counting_type: COUNTING_TYPE,
        proposal_threshold: 1,
        vote_delay: 0,
        vote_period: 720,
        timelock: 0,
        grace_period: 17280,
    };

/// One row per seeded Script3 proposal. `vote_start`/`vote_end`/tallies are the
/// values in the committed `get_proposal` / `get_proposal_votes` captures.
struct Expected {
    id: u32,
    suffix: &'static str,
    title: &'static str,
    action: Action,
    executable: bool,
    vote_start: u32,
    vote_end: u32,
    eta: u32,
    /// Expected transfer amount for `CalldataTransfer` rows, straight from
    /// the captured `Calldata.args[2]`; `None` for non-transfer actions.
    amount: Option<i128>,
    against: i128,
    for_votes: i128,
    abstain: i128,
    outcome: VoteOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    CalldataTransfer,
    /// s4: a call to a contract outside the governance surface (`no_auth_sc`
    /// on the mock subcall contract) — decoded positionally, no amount.
    CalldataUnknown,
    Upgrade,
    Council,
}

fn expectations() -> Vec<Expected> {
    vec![
        Expected {
            id: 0,
            suffix: "read-s3-get_proposal-s1-transfer-executed",
            title: "Seed v2: treasury transfer to delegate-1",
            action: Action::CalldataTransfer,
            executable: true,
            vote_start: 5083633,
            vote_end: 5084353,
            eta: 0,
            amount: Some(10_000_000_000),
            against: 0,
            for_votes: 7_500_000_000_000,
            abstain: 0,
            outcome: VoteOutcome::Successful,
        },
        Expected {
            id: 1,
            suffix: "read-s3-get_proposal-s2-contract-upgrade",
            title: "Seed v2: governor code upgrade",
            action: Action::Upgrade,
            executable: true,
            vote_start: 5083634,
            vote_end: 5084354,
            eta: 0,
            amount: None,
            against: 0,
            for_votes: 3_000_000_000_000,
            abstain: 0,
            outcome: VoteOutcome::Successful,
        },
        Expected {
            id: 2,
            suffix: "read-s3-get_proposal-s3-admin-change",
            title: "Seed v2: council seat change",
            action: Action::Council,
            executable: true,
            vote_start: 5083635,
            vote_end: 5084355,
            eta: 0,
            amount: None,
            against: 0,
            for_votes: 4_500_000_000_000,
            abstain: 0,
            outcome: VoteOutcome::Successful,
        },
        Expected {
            id: 3,
            suffix: "read-s3-get_proposal-s4-unknown-contract-call",
            title: "Seed v2: call outside the governance surface",
            action: Action::CalldataUnknown,
            executable: true,
            vote_start: 5083636,
            vote_end: 5084356,
            eta: 0,
            amount: None,
            against: 0,
            for_votes: 2_000_000_000_000,
            abstain: 0,
            outcome: VoteOutcome::Successful,
        },
        Expected {
            id: 4,
            suffix: "read-s3-get_proposal-s5-failing-quorum",
            title: "Seed v2: quorum failure probe",
            action: Action::CalldataTransfer,
            executable: true,
            vote_start: 5083637,
            vote_end: 5084357,
            eta: 0,
            amount: Some(2_500_000_000),
            against: 0,
            for_votes: 0,
            abstain: 0,
            outcome: VoteOutcome::NoQuorum,
        },
        Expected {
            id: 5,
            suffix: "read-s3-get_proposal-s6-abstain-heavy",
            title: "Seed v2: abstain-heavy probe",
            action: Action::CalldataTransfer,
            executable: true,
            vote_start: 5083638,
            vote_end: 5084358,
            eta: 0,
            amount: Some(5_000_000_000),
            against: 0,
            for_votes: 0,
            abstain: 7_500_000_000_000,
            outcome: VoteOutcome::ThresholdNotMet,
        },
    ]
}

fn adapter() -> Script3Adapter {
    Script3Adapter
}

// --- the differential tests ---------------------------------------------------

#[test]
fn corpus_covers_every_seeded_proposal_id() {
    let ids: Vec<u32> = expectations().iter().map(|e| e.id).collect();
    assert_eq!(ids, vec![0, 1, 2, 3, 4, 5], "ids 0-5 must all be covered");
}

#[test]
fn every_proposal_state_matches_the_captured_on_chain_read() {
    for e in expectations() {
        let state = adapter()
            .state_from_chain(e.id, &decoded_return(e.suffix))
            .unwrap_or_else(|err| panic!("proposal {} failed to decode: {err}", e.id));

        assert_eq!(state.proposal_id, e.id, "{} id", e.suffix);
        assert_eq!(state.governor, "script3-soroban-governor");
        assert_eq!(state.title, e.title, "{} title", e.suffix);
        assert_eq!(state.executable, e.executable, "{} executable", e.suffix);
        assert_eq!(state.vote_start, e.vote_start, "{} vote_start", e.suffix);
        assert_eq!(state.vote_end, e.vote_end, "{} vote_end", e.suffix);
        assert_eq!(state.eta, e.eta, "{} eta", e.suffix);
        assert_eq!(
            state.status,
            ProposalStatus::Open,
            "{} status is still Open in the captures",
            e.suffix
        );
        assert!(
            state.creator.starts_with('G'),
            "{} creator must be an account address, got {:?}",
            e.suffix,
            state.creator
        );

        match (e.action, &state.action) {
            (Action::CalldataTransfer, Script3Action::Calldata(cd)) => {
                assert_eq!(cd.function, "transfer", "{} action function", e.suffix);
                assert_eq!(cd.args.len(), 3, "{} transfer arity", e.suffix);
                assert_eq!(
                    cd.contract_id, "CCAUJK6V6GIYQANKCV2JDCMCLGMQD42JCF426TQHHMSXW2F5ZROM4IKY",
                    "{} target must be the votes contract",
                    e.suffix
                );
                assert!(cd.auths.is_empty(), "{} has no nested auths", e.suffix);
                assert_eq!(cd.amount(), e.amount, "{} transfer amount", e.suffix);
            }
            (Action::CalldataUnknown, Script3Action::Calldata(cd)) => {
                assert_eq!(cd.function, "no_auth_sc", "{} action function", e.suffix);
                assert_eq!(cd.args.len(), 1, "{} arity", e.suffix);
                assert_eq!(
                    cd.contract_id, "CAQCXFI6YSXWGCB37PRZVG5YNLLJMY45IWLUUTFMTXFIAF2MOZ5DMGJ7",
                    "{} target must be the mock subcall contract",
                    e.suffix
                );
                assert!(cd.auths.is_empty(), "{} has no nested auths", e.suffix);
                assert!(
                    cd.amount().is_none(),
                    "{} an unknown function must not produce an amount",
                    e.suffix
                );
            }
            (Action::Upgrade, Script3Action::Upgrade(_)) => {}
            (Action::Council, Script3Action::Council(addr)) => {
                assert!(addr.starts_with('G'), "{} council address", e.suffix);
            }
            (want, got) => panic!(
                "{} action variant mismatch: expected {want:?}, decoded {got:?}",
                e.suffix
            ),
        }
    }
}

#[test]
fn every_tally_matches_the_captured_on_chain_read() {
    for e in expectations() {
        let suffix = e.suffix.replace("get_proposal-", "get_proposal_votes-");
        let tally = adapter()
            .tally_from_chain(&decoded_return(&suffix))
            .unwrap_or_else(|err| panic!("{} tally failed to decode: {err}", e.suffix));
        assert_eq!(tally.against, e.against, "{} against", e.suffix);
        assert_eq!(tally.for_votes, e.for_votes, "{} for", e.suffix);
        assert_eq!(tally.abstain, e.abstain, "{} abstain", e.suffix);
    }
}

#[test]
fn replica_outcome_matches_the_expected_verdict_for_every_proposal() {
    for e in expectations() {
        let suffix = e.suffix.replace("get_proposal-", "get_proposal_votes-");
        let tally = adapter()
            .tally_from_chain(&decoded_return(&suffix))
            .expect("decodes");
        assert_eq!(
            evaluate(&tally, &SETTINGS, TOTAL_SUPPLY),
            e.outcome,
            "{} outcome",
            e.suffix
        );
    }
}

#[test]
fn the_two_failure_modes_are_distinguished() {
    // s5 has no votes at all; s6 has votes but none of them are `for`. These
    // fail for genuinely different reasons and a replica that collapsed them
    // would be wrong in a way a governance UI would show as "defeated, cause
    // unknown".
    let by_id = |id: u32| -> VoteCount {
        let e = expectations()
            .into_iter()
            .find(|e| e.id == id)
            .expect("known id");
        let suffix = e.suffix.replace("get_proposal-", "get_proposal_votes-");
        adapter()
            .tally_from_chain(&decoded_return(&suffix))
            .expect("decodes")
    };
    assert_eq!(
        evaluate(&by_id(4), &SETTINGS, TOTAL_SUPPLY),
        VoteOutcome::NoQuorum
    );
    assert_eq!(
        evaluate(&by_id(5), &SETTINGS, TOTAL_SUPPLY),
        VoteOutcome::ThresholdNotMet
    );
    // s6 reached quorum only because counting_type includes abstain.
    assert!(by_id(5).is_over_quorum(QUORUM_BPS, COUNTING_TYPE, TOTAL_SUPPLY));
    assert!(!by_id(5).is_over_threshold(VOTE_THRESHOLD_BPS));
}

#[test]
fn quorum_requirement_matches_the_governor_parameters() {
    // floor(total_supply * quorum_bps / BPS_SCALAR) with the live numbers.
    let expected = (TOTAL_SUPPLY * i128::from(QUORUM_BPS)) / BPS_SCALAR;
    assert_eq!(expected, 155_000_000_000);
    // The smallest corpus tally that clears quorum.
    let s4 = expectations().into_iter().find(|e| e.id == 3).expect("s4");
    let tally = VoteCount::new(s4.against, s4.for_votes, s4.abstain);
    assert!(tally.is_over_quorum(QUORUM_BPS, COUNTING_TYPE, TOTAL_SUPPLY));
    let s5 = expectations().into_iter().find(|e| e.id == 4).expect("s5");
    let empty = VoteCount::new(s5.against, s5.for_votes, s5.abstain);
    assert!(!empty.is_over_quorum(QUORUM_BPS, COUNTING_TYPE, TOTAL_SUPPLY));
}

#[test]
fn voted_amounts_reconstruct_the_captured_tallies() {
    // Independent cross-check: summing the recorded per-vote amounts must equal
    // the tally the chain reports. Per-vote amounts are the power-at-snapshot
    // reads captured by scripts/seed-v2/snapshot-power.js (see the test below),
    // NOT uniform values — s1's voters held 300k/250k/200k base units at the
    // snapshot ledger, and the sum must still equal the captured tally.
    let s1: &[(u32, i128)] = &[
        (1, 3_000_000_000_000),
        (1, 2_500_000_000_000),
        (1, 2_000_000_000_000),
    ];
    let mut folded = VoteCount::default();
    for &(support, amount) in s1 {
        folded.add_vote(support, amount).expect("valid support");
    }
    assert_eq!(folded.for_votes, 7_500_000_000_000);

    let s6: &[(u32, i128)] = &[
        (2, 3_000_000_000_000),
        (2, 2_500_000_000_000),
        (2, 2_000_000_000_000),
    ];
    let mut folded = VoteCount::default();
    for &(support, amount) in s6 {
        folded.add_vote(support, amount).expect("valid support");
    }
    assert_eq!(folded.abstain, 7_500_000_000_000);
}

/// Power-at-snapshot differential (N3 exit criterion).
///
/// For every Script3 proposal, folding the captured
/// `get_past_votes(voter, vote_start)` reads — by the support each voter cast,
/// from the committed proposals definition — must equal the captured
/// `get_proposal_votes` tally, and the captured `get_past_total_supply` must
/// equal the quorum denominator used by the replica. All inputs are committed
/// captures; nothing here reaches the network.
///
/// Expected powers are hand-derived from the committed mint constants
/// (`scripts/seed-v2/lib/config.js`: 300k/250k/200k/150k/100k/50k base units
/// × 1e7): the snapshot ledger equals each proposal's creation ledger, and the
/// seed run created every proposal BEFORE any delegation, so snapshot power is
/// the voter's own balance. The delegations (delegate-4→1, delegate-5→2) must
/// therefore NOT appear in these reads — a non-zero delegated contribution
/// would mean the snapshot moved, and this test fails.
#[test]
fn power_at_snapshot_reconstructs_every_captured_tally() {
    /// (proposal key suffix, snapshot vote_start, voters as
    /// (fixture name part, support, expected power)).
    let rows: &[(&str, u32, &[(&str, u32, i128)])] = &[
        (
            "s1-transfer-executed",
            5083633,
            &[
                ("sgg-delegate-1", 1, 3_000_000_000_000),
                ("sgg-delegate-2", 1, 2_500_000_000_000),
                ("sgg-delegate-3", 1, 2_000_000_000_000),
            ],
        ),
        ("s2-contract-upgrade", 5083634, &[("sgg-delegate-1", 1, 3_000_000_000_000)]),
        (
            "s3-admin-change",
            5083635,
            &[
                ("sgg-delegate-2", 1, 2_500_000_000_000),
                ("sgg-delegate-3", 1, 2_000_000_000_000),
            ],
        ),
        ("s4-unknown-contract-call", 5083636, &[("sgg-delegate-3", 1, 2_000_000_000_000)]),
        ("s5-failing-quorum", 5083637, &[]),
        (
            "s6-abstain-heavy",
            5083638,
            &[
                ("sgg-delegate-1", 2, 3_000_000_000_000),
                ("sgg-delegate-2", 2, 2_500_000_000_000),
                ("sgg-delegate-3", 2, 2_000_000_000_000),
            ],
        ),
    ];

    for (key, vote_start, voters) in rows {
        // Quorum denominator at the snapshot ledger: hand-derived from the
        // committed mints (50k+300k+250k+200k+150k+100k+500k treasury) × 1e7.
        let supply = decoded_return(&format!("read-s3-get_past_total_supply-{key}"));
        let supply = soroban_governance_core::scval::scval_i128(&supply)
            .unwrap_or_else(|| panic!("{key} snapshot supply must be i128"));
        assert_eq!(
            supply, TOTAL_SUPPLY,
            "{key} captured get_past_total_supply must equal the quorum denominator"
        );
        assert_eq!(
            supply,
            (50_000 + 300_000 + 250_000 + 200_000 + 150_000 + 100_000 + 500_000)
                * 10_000_000,
            "{key} supply must equal the committed mint constants"
        );

        // Per-voter power at the snapshot, read back from the captures and
        // cross-checked against the hand-derived mint values.
        let mut folded = VoteCount::default();
        for (voter, support, expected_power) in *voters {
            let read = decoded_return(&format!(
                "read-s3-get_past_votes-{key}-{voter}"
            ));
            let power = soroban_governance_core::scval::scval_i128(&read)
                .unwrap_or_else(|| panic!("{key}/{voter} snapshot power must be i128"));
            assert_eq!(
                power, *expected_power,
                "{key}/{voter} power at vote_start {vote_start}"
            );
            folded
                .add_vote(*support, power)
                .expect("valid support from the committed proposal definition");
        }

        // The folded snapshot powers must equal the captured on-chain tally.
        let suffix = format!("read-s3-get_proposal_votes-{key}");
        let on_chain = adapter()
            .tally_from_chain(&decoded_return(&suffix))
            .unwrap_or_else(|err| panic!("{key} tally failed to decode: {err}"));
        assert_eq!(
            folded, on_chain,
            "{key}: folded power-at-snapshot reads must equal the captured tally"
        );
    }
}

#[test]
fn contextual_rules_fire_on_the_real_s1_treasury_transfer() {
    // s1 moves 10 GOV out of the governor's own treasury: the governor is
    // args[0] of `transfer(from, to, amount)` on the votes contract — the
    // CALL TARGET is the votes contract, NOT the governor. So treasury_outflow
    // and large_value fire on genuine captured data, while self_call must NOT:
    // asserting that keeps the rule honest about what the capture proves.
    let state = adapter()
        .state_from_chain(
            0,
            &decoded_return("read-s3-get_proposal-s1-transfer-executed"),
        )
        .expect("decodes");
    let Script3Action::Calldata(cd) = &state.action else {
        panic!("s1 must be a Calldata action");
    };
    assert_eq!(
        cd.contract_id, "CCAUJK6V6GIYQANKCV2JDCMCLGMQD42JCF426TQHHMSXW2F5ZROM4IKY",
        "s1 targets the votes contract, not the governor"
    );

    let context = RiskContext {
        governor: Some(GOVERNOR_CONTRACT_ID.to_owned()),
        treasury_addresses: vec![GOVERNOR_CONTRACT_ID.to_owned()],
        large_value_threshold: Some(1),
    };
    let flags = context.evaluate(&CallUnderReview {
        target: cd.contract_id.clone(),
        function: cd.function.clone(),
        args: cd.args.clone(),
        amount: cd.amount(),
        nested: vec![],
    });

    assert!(
        flags.fired("treasury_outflow"),
        "s1 moves funds OUT of the governor treasury (args[0])"
    );
    assert!(
        !flags.fired("self_call"),
        "s1 targets the votes contract — the governor is the source, not the target"
    );
    assert!(flags.fired("large_value"), "threshold set to 1 wei");
    assert_eq!(flags.highest_tier(), RiskTier::Critical);
    for f in &flags.flags {
        assert!(!f.evidence.is_null(), "{} must carry evidence", f.rule);
    }
}

#[test]
fn decoded_proposal_is_unverified_for_the_target_contract_args() {
    // We have no contractspec for the votes contract in this test, so argument
    // names must NOT be invented: decoding stays Unverified.
    let state = adapter()
        .state_from_chain(
            0,
            &decoded_return("read-s3-get_proposal-s1-transfer-executed"),
        )
        .expect("decodes");
    let ctx = RiskContext::default();
    let proposal = adapter().decode_proposal(&state, &ctx).expect("decodes");
    assert_eq!(proposal.governor, "script3-soroban-governor");
    assert_eq!(proposal.proposal_id, "0");
    assert_eq!(proposal.calls.len(), 1);
    let call = &proposal.calls[0];
    assert_eq!(call.function_name, "transfer");
    assert_eq!(
        call.decoding,
        DecodingStatus::Unverified,
        "target-contract args must not be presented as verified"
    );
    // Positional args, no invented names.
    assert!(call.arguments.is_array(), "args stay positional");
}

#[test]
fn upgrade_proposal_is_decoded_as_a_wasm_hash_not_a_call() {
    let state = adapter()
        .state_from_chain(
            1,
            &decoded_return("read-s3-get_proposal-s2-contract-upgrade"),
        )
        .expect("decodes");
    assert!(matches!(state.action, Script3Action::Upgrade(_)));
    let ctx = RiskContext::default();
    let proposal = adapter().decode_proposal(&state, &ctx).expect("decodes");
    assert_eq!(proposal.calls[0].function_name, "upgrade");
    assert_eq!(proposal.calls[0].risk.tier, RiskTier::Critical);
    assert!(
        proposal.calls[0].risk.verified,
        "variant name is from the pinned enum"
    );
}

#[test]
fn vote_power_model_is_recorded_and_stable() {
    assert_eq!(
        adapter().vote_power_model(),
        VotePowerModel::CheckpointedVotesContract
    );
    assert_eq!(
        adapter().vote_power_model().as_str(),
        "checkpointed_votes_contract"
    );
}

#[test]
fn captured_corpus_is_the_committed_one() {
    // Guard against the differential tests silently degrading to zero coverage
    // if the fixture set is ever pruned.
    let dir = fixture_dir().join("rpc");
    let count = std::fs::read_dir(&dir)
        .expect("rpc dir")
        .filter_map(Result::ok)
        .filter(|e| {
            // ASCII-digit prefix keeps captures (0001-…) apart from README.md.
            let name = e.file_name().to_string_lossy().into_owned();
            let is_json = std::path::Path::new(&name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
            is_json && name.starts_with(|c: char| c.is_ascii_digit())
        })
        .count();
    assert!(
        count >= 232,
        "expected the full committed corpus (>=232 captures), found {count}"
    );
    let index = std::fs::read_to_string(fixture_dir().join("index.json")).expect("index.json");
    let parsed: serde_json::Value = serde_json::from_str(&index).expect("index.json parses");
    assert_eq!(
        parsed["fixtures"].as_array().map(Vec::len),
        Some(count),
        "index.json must list exactly the committed captures"
    );
}
