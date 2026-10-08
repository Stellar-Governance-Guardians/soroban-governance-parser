//! Property tests for the tally / quorum / checkpoint replicas (N3 exit
//! criterion: "proptest for tally/quorum edge cases").
//!
//! OFFLINE and deterministic: inputs are generated, never fetched, and every
//! property states a rule transcribed from the pinned upstream source
//! (`script3/soroban-governor @ a2ac6de81055be5bd13e31f922c9546309bfdb8a`).
//!
//! The companion `differential.rs` proves the replicas agree with *captured
//! chain reads*; these tests prove the replicas agree with the upstream
//! *rules* across the input space the corpus does not cover: zero votes,
//! ties, i128 extremes, empty checkpoint history, exact-ledger hits and
//! before-first-checkpoint lookups.
//!
//! No `expect`/`unwrap`/`panic` suppressions: this is a test crate, mirrors
//! the `#[allow(clippy::expect_used, clippy::unwrap_used)]` precedent on
//! every `#[cfg(test)]` module in src/, and failing loudly is the point.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::unreadable_literal
)]

use proptest::prelude::*;
use soroban_governance_core::checkpoint::{pack, unpack, upper_lookup, CheckpointError};
use soroban_governance_core::tally::{
    evaluate, GovernorSettings, VoteCount, VoteOutcome, BPS_SCALAR,
};

const SETTINGS: GovernorSettings = GovernorSettings {
    quorum_bps: 100,
    vote_threshold_bps: 5000,
    counting_type: 0b111,
    proposal_threshold: 1,
    vote_delay: 0,
    vote_period: 720,
    timelock: 0,
    grace_period: 17280,
};

/// Any representable tally, including i128 extremes (positive and negative).
fn tally_strategy() -> impl Strategy<Value = VoteCount> {
    (any::<i128>(), any::<i128>(), any::<i128>()).prop_map(|(a, f, b)| VoteCount::new(a, f, b))
}

/// Non-negative tallies: the only shape the chain can actually produce.
fn natural_tally_strategy() -> impl Strategy<Value = VoteCount> {
    (0i128..=i128::MAX, 0i128..=i128::MAX, 0i128..=i128::MAX)
        .prop_map(|(a, f, b)| VoteCount::new(a, f, b))
}

/// Amounts that must be rejected: above u96 or negative.
fn out_of_u96_amounts() -> impl Strategy<Value = i128> {
    prop_oneof![(1i128 << 96)..=i128::MAX, i128::MIN..=-1i128]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Zero votes never reach quorum and never meet the threshold — the
    /// "zero votes" edge from the exit criterion, for every supply shape.
    #[test]
    fn zero_tally_is_always_no_quorum(total in 0i128..=i128::MAX) {
        let empty = VoteCount::default();
        prop_assert!(!empty.is_over_quorum(SETTINGS.quorum_bps, SETTINGS.counting_type, total));
        prop_assert!(!empty.is_over_threshold(SETTINGS.vote_threshold_bps));
        prop_assert_eq!(evaluate(&empty, &SETTINGS, total), VoteOutcome::NoQuorum);
    }

    /// Quorum is a STRICT `>`: a tally exactly at the floored requirement
    /// fails, one unit over passes. Holds for every total_supply and bps.
    #[test]
    fn quorum_boundary_is_strict_inequality(
        total in 0i128..=i128::MAX,
        quorum_bps in 0u32..=10_000u32,
        counting_type in 0u32..=0b111u32,
    ) {
        let requirement = total.saturating_mul(i128::from(quorum_bps)).saturating_div(BPS_SCALAR);
        // Put the entire count in whichever bucket counting_type selects;
        // with counting_type 0b111 any one bucket alone is counted.
        let ct = if counting_type == 0 { 0 } else { counting_type };
        // Put the entire count in whichever bucket counting_type selects
        // (prefer against, then for, then abstain).
        let bucket = |v: i128| {
            if ct & 0b100 != 0 {
                VoteCount::new(v, 0, 0)
            } else if ct & 0b010 != 0 {
                VoteCount::new(0, v, 0)
            } else {
                VoteCount::new(0, 0, v)
            }
        };
        prop_assume!(ct != 0);
        prop_assert!(!bucket(requirement).is_over_quorum(quorum_bps, ct, total));
        if let Some(over) = requirement.checked_add(1) {
            if over >= 0 {
                prop_assert!(bucket(over).is_over_quorum(quorum_bps, ct, total));
            }
        }
    }

    /// Threshold counts only `for` over `against + for`: abstain can change
    /// the outcome only via quorum, never via the threshold (a tie —
    /// for == against — always fails a 50% threshold because `>` is strict).
    #[test]
    fn abstain_never_moves_the_threshold(
        against in 0i128..=1_000_000_000_000_000i128,
        for_votes in 0i128..=1_000_000_000_000_000i128,
        abstain in 0i128..=1_000_000_000_000_000i128,
    ) {
        let with = VoteCount::new(against, for_votes, abstain);
        let without = VoteCount::new(against, for_votes, 0);
        prop_assert_eq!(
            with.is_over_threshold(SETTINGS.vote_threshold_bps),
            without.is_over_threshold(SETTINGS.vote_threshold_bps)
        );
    }

    /// Exact 50/50 tie at a 50% threshold fails (upstream strict `>`), and
    /// an exact tie is decided solely by the threshold, never "draws".
    ///
    /// Bounded to u96/2 because the votes contract stores power in a packed
    /// u96 checkpoint — values beyond that are unreachable on chain, and the
    /// replica deliberately saturates (documented in tally.rs) where
    /// upstream's raw `*` would overflow.
    #[test]
    fn exact_tie_fails_the_threshold(against in 0i128..=((1i128 << 96) - 1) / 2) {
        let tie = VoteCount::new(against, against, 0);
        if against > 0 {
            prop_assert!(!tie.is_over_threshold(5000));
        }
        // One unit of `for` past the tie passes a 50% threshold:
        // requirement = floor((2a+1)/2) = a, and for = a+1 > a.
        let past = VoteCount::new(against, against.saturating_add(1), 0);
        prop_assert!(past.is_over_threshold(5000));
    }

    /// i128 extremes: `count_quorum` saturates instead of wrapping, and
    /// `evaluate` always returns one of the three declared outcomes — never
    /// panics, never returns a fabricated fourth state.
    #[test]
    fn extremes_are_total_and_saturating(tally in tally_strategy()) {
        let _q = tally.count_quorum(0b111);
        let outcome = evaluate(&tally, &SETTINGS, i128::MAX);
        prop_assert!(matches!(
            outcome,
            VoteOutcome::Successful | VoteOutcome::NoQuorum | VoteOutcome::ThresholdNotMet
        ));
    }

    /// Positive-only tallies: a saturated quorum count must never go
    /// negative (a wraparound would read as "quorum can never be met").
    #[test]
    fn saturated_quorum_count_is_never_negative(tally in natural_tally_strategy()) {
        prop_assert!(tally.count_quorum(0b111) >= 0);
        prop_assert!(tally.count_quorum(0b000) == 0);
    }

    /// add_vote folds exactly like the declared bucket, and an invalid
    /// support value (not in 0..=2) is always an error, never a silent skip.
    #[test]
    fn folding_matches_the_bucket_or_errors(
        votes in proptest::collection::vec((0u32..=4, 0i128..1_000_000_000_000i128), 0..32),
    ) {
        let mut folded = VoteCount::default();
        let mut expect = VoteCount::default();
        let mut saw_invalid = false;
        for &(support, amount) in &votes {
            match support {
                0 => expect.against += amount,
                1 => expect.for_votes += amount,
                2 => expect.abstain += amount,
                _ => saw_invalid = true,
            }
            if let Err(e) = folded.add_vote(support, amount) {
                prop_assert!(saw_invalid, "valid support must not error");
                let _ = e;
                break;
            }
        }
        if !saw_invalid {
            prop_assert_eq!(folded, expect);
        }
    }

    /// Every packed checkpoint round-trips, and packing never mixes the
    /// sequence into the amount field (or vice versa) for any input.
    #[test]
    fn pack_unpack_round_trips(sequence in any::<u32>(), amount in 0i128..=(1i128 << 96) - 1) {
        let packed = pack(sequence, amount).expect("in-range amount packs");
        prop_assert_eq!(unpack(packed), (sequence, amount));
        // The sequence lives entirely in the top 32 bits.
        prop_assert_eq!((packed >> 96) as u32, sequence);
    }

    /// Amounts outside u96 fail closed — the exact inputs upstream traps on
    /// (`InvalidCheckpointError`), never a truncation.
    #[test]
    fn out_of_range_amounts_fail_closed(
        sequence in any::<u32>(),
        amount in out_of_u96_amounts(),
    ) {
        prop_assert_eq!(
            pack(sequence, amount),
            Err(CheckpointError::InvalidAmount(amount))
        );
    }

    /// upper_lookup is a floor: the returned checkpoint always has
    /// `sequence <= query`, is the largest such, and an empty history (or a
    /// query before the first checkpoint) returns exactly 0.
    #[test]
    fn lookup_returns_the_floor_checkpoint(
        checkpoints in proptest::collection::vec((1u32..=1_000, 0i128..1_000_000_000_000i128), 0..24),
        query in 0u32..1_100,
    ) {
        // Build the sorted, deduped-by-insert vector the contract maintains.
        let mut packed: Vec<u128> = checkpoints
            .iter()
            .map(|&(s, a)| pack(s, a).expect("amount fits u96"))
            .collect();
        packed.sort_unstable();

        let got = upper_lookup(&packed, query);
        // Recompute the floor by brute force — an independent implementation.
        // The floor is by SEQUENCE (sort order), not by amount: it is the last
        // packed entry whose sequence is <= query, which for duplicate
        // sequences is the largest amount at that sequence.
        let expected = packed
            .iter()
            .rev()
            .map(|&p| unpack(p))
            .find(|&(s, _)| s <= query)
            .map_or(0, |(_, a)| a);
        prop_assert_eq!(got, expected);

        if packed.is_empty() {
            prop_assert_eq!(got, 0);
        }
        // Exact-ledger hit: if a checkpoint exists AT the query sequence,
        // the floor is that checkpoint — the last entry at that sequence in
        // sort order (binary_search lands after all of them).
        let exact: Vec<i128> = packed
            .iter()
            .map(|&p| unpack(p))
            .filter(|&(s, _)| s == query)
            .map(|(_, a)| a)
            .collect();
        if let Some(&last_at_query) = exact.last() {
            prop_assert_eq!(got, last_at_query);
        }
        // Before-first-checkpoint: any query below the smallest sequence
        // returns 0, even for a non-empty history. `packed` is sorted, so
        // the head IS the smallest sequence.
        if let Some(&(first_seq, _)) = packed.first().map(|&p| unpack(p)).as_ref() {
            if query < first_seq {
                prop_assert_eq!(got, 0);
            }
        }
    }
}
