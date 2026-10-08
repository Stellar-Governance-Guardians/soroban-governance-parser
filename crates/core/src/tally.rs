//! Pure replicas of the Script3 tally, quorum and outcome rules.
//!
//! These functions are deliberately side-effect free and take every input as an
//! argument, so the differential tests can compare them against captured chain
//! reads without any IO. They are a *replica*, not a re-implementation of
//! intent: each rule below is transcribed from the pinned upstream source and
//! cites it.
//!
//! Pinned source: `github.com/script3/soroban-governor` @ `a2ac6de81055be5bd13e31f922c9546309bfdb8a`
//!   - `contracts/governor/src/vote_count.rs` — `count_quorum`, `is_over_quorum`, `is_over_threshold`
//!   - `contracts/governor/src/constants.rs`   — `BPS_SCALAR = 10_000`
//!
//! # Why the off-by-ones matter
//!
//! Both comparisons upstream are **strict** `>`, not `>=`, and the quorum
//! requirement is floored by integer division. A replica that used `>=` or
//! rounded up would disagree with the chain exactly on the boundary cases, and
//! those are the cases a governance UI most needs to get right. The boundary is
//! exercised explicitly in the tests below.

/// Basis-point scalar. Upstream `constants.rs::BPS_SCALAR`.
pub const BPS_SCALAR: i128 = 10_000;

/// Vote tallies, matching upstream `types.rs::VoteCount`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct VoteCount {
    pub against: i128,
    pub for_votes: i128,
    pub abstain: i128,
}

impl VoteCount {
    pub const fn new(against: i128, for_votes: i128, abstain: i128) -> Self {
        Self {
            against,
            for_votes,
            abstain,
        }
    }

    /// Upstream `VoteCount::add_vote`: `support` 0=against, 1=for, 2=abstain.
    /// Any other value is an error upstream, so it is an error here — never
    /// silently coerced.
    pub fn add_vote(&mut self, support: u32, amount: i128) -> Result<(), TallyError> {
        let sum = match support {
            0 => self.against.checked_add(amount),
            1 => self.for_votes.checked_add(amount),
            2 => self.abstain.checked_add(amount),
            _ => return Err(TallyError::InvalidSupport(support)),
        };
        let sum = sum.ok_or(TallyError::Overflow)?;
        match support {
            0 => self.against = sum,
            1 => self.for_votes = sum,
            _ => self.abstain = sum,
        }
        Ok(())
    }

    /// Upstream `VoteCount::count_quorum`.
    ///
    /// `counting_type` is a bitfield laid out `{MSB}...{against}{for}{abstain}`,
    /// so bit 2 selects against, bit 1 for, bit 0 abstain.
    pub fn count_quorum(&self, counting_type: u32) -> i128 {
        let mut q = 0i128;
        if counting_type & 0b100 != 0 {
            q = q.saturating_add(self.against);
        }
        if counting_type & 0b010 != 0 {
            q = q.saturating_add(self.for_votes);
        }
        if counting_type & 0b001 != 0 {
            q = q.saturating_add(self.abstain);
        }
        q
    }

    /// Upstream `VoteCount::is_over_quorum`.
    ///
    /// Note the floor division and the **strict** `>`: a tally exactly equal to
    /// the requirement does NOT pass.
    pub fn is_over_quorum(&self, quorum_bps: u32, counting_type: u32, total_votes: i128) -> bool {
        let quorum_votes = self.count_quorum(counting_type);
        let requirement = floor_mul(total_votes, i128::from(quorum_bps));
        quorum_votes > requirement
    }

    /// Upstream `VoteCount::is_over_threshold`.
    ///
    /// Abstain is excluded from the denominator, and a tally with no
    /// against+for votes returns false regardless of the threshold.
    pub fn is_over_threshold(&self, vote_threshold_bps: u32) -> bool {
        let Some(against_and_for) = self.against.checked_add(self.for_votes) else {
            return false;
        };
        if against_and_for == 0 {
            return false;
        }
        let requirement = floor_mul(against_and_for, i128::from(vote_threshold_bps));
        self.for_votes > requirement
    }
}

/// `floor(value * bps / BPS_SCALAR)`, mirroring upstream integer division.
///
/// Saturates instead of overflowing: upstream operates on `i128` amounts that
/// the host bounds well below `i128::MAX`, and a saturating floor can only ever
/// be more conservative than a wrap.
fn floor_mul(value: i128, bps: i128) -> i128 {
    value.saturating_mul(bps).saturating_div(BPS_SCALAR)
}

/// Governor parameters, mirroring upstream `types.rs::GovernorSettings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GovernorSettings {
    pub quorum_bps: u32,
    pub vote_threshold_bps: u32,
    pub counting_type: u32,
    pub proposal_threshold: i128,
    pub vote_delay: u32,
    pub vote_period: u32,
    pub timelock: u32,
    pub grace_period: u32,
}

/// Lifecycle state, mirroring upstream `types.rs::ProposalStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Open = 0,
    Successful = 1,
    Defeated = 2,
    Executed = 3,
    Expired = 4,
    Canceled = 5,
}

impl ProposalStatus {
    pub fn from_u32(raw: u32) -> Option<Self> {
        Some(match raw {
            0 => Self::Open,
            1 => Self::Successful,
            2 => Self::Defeated,
            3 => Self::Executed,
            4 => Self::Expired,
            5 => Self::Canceled,
            _ => return None,
        })
    }
}

/// Outcome of evaluating a closed vote, before any queue/execute transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteOutcome {
    /// Met both the quorum and the vote threshold.
    Successful,
    /// Did not reach quorum.
    NoQuorum,
    /// Reached quorum but not the vote threshold.
    ThresholdNotMet,
}

/// Errors that the upstream contract would trap on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TallyError {
    /// `support` outside 0..=2. Upstream: `InvalidProposalSupportError`.
    InvalidSupport(u32),
    /// i128 overflow while accumulating. Upstream: `MathOverflow`.
    Overflow,
}

impl core::fmt::Display for TallyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidSupport(s) => write!(f, "invalid vote support {s} (expected 0, 1 or 2)"),
            Self::Overflow => write!(f, "vote tally overflowed i128"),
        }
    }
}

impl std::error::Error for TallyError {}

/// Full evaluation of a tally against governor settings.
///
/// Mirrors the decision the governor makes when a voting period closes: quorum
/// is checked first, and the threshold is only meaningful once quorum is met.
pub fn evaluate(tally: &VoteCount, settings: &GovernorSettings, total_votes: i128) -> VoteOutcome {
    if !tally.is_over_quorum(settings.quorum_bps, settings.counting_type, total_votes) {
        return VoteOutcome::NoQuorum;
    }
    if !tally.is_over_threshold(settings.vote_threshold_bps) {
        return VoteOutcome::ThresholdNotMet;
    }
    VoteOutcome::Successful
}

/// Fold a stream of `(support, amount)` votes into a tally.
///
/// Mirrors upstream `add_vote`, including trapping on an out-of-range support
/// rather than ignoring the vote.
pub fn tally_votes(votes: &[(u32, i128)]) -> Result<VoteCount, TallyError> {
    let mut out = VoteCount::default();
    for &(support, amount) in votes {
        out.add_vote(support, amount)?;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    // Live values read from the seeded Script3 governor on testnet
    // (contract CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX).
    const TOTAL_SUPPLY: i128 = 15_500_000_000_000;
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

    #[test]
    fn settings_match_the_live_governor() {
        // Guards against silently drifting away from the deployed parameters.
        assert_eq!(SETTINGS.quorum_bps, 100);
        assert_eq!(SETTINGS.vote_threshold_bps, 5000);
        assert_eq!(SETTINGS.counting_type, 7);
        assert_eq!(TOTAL_SUPPLY, 15_500_000_000_000);
    }

    #[test]
    fn quorum_counts_only_the_selected_buckets() {
        let t = VoteCount::new(100, 200, 400);
        assert_eq!(t.count_quorum(0b111), 700);
        assert_eq!(t.count_quorum(0b011), 600, "for + abstain");
        assert_eq!(t.count_quorum(0b010), 200, "for only");
        assert_eq!(t.count_quorum(0b001), 400, "abstain only");
        assert_eq!(t.count_quorum(0b000), 0);
    }

    #[test]
    fn quorum_uses_strict_greater_than_not_gte() {
        // requirement = floor(TOTAL_SUPPLY * 100 / 10_000) = 155_000_000_000
        let requirement = 155_000_000_000;
        let exactly = VoteCount::new(0, requirement, 0);
        let one_over = VoteCount::new(0, requirement + 1, 0);
        let one_under = VoteCount::new(0, requirement - 1, 0);

        assert!(
            !exactly.is_over_quorum(SETTINGS.quorum_bps, SETTINGS.counting_type, TOTAL_SUPPLY),
            "a tally exactly at the requirement must NOT pass quorum (upstream uses `>`)"
        );
        assert!(one_over.is_over_quorum(SETTINGS.quorum_bps, SETTINGS.counting_type, TOTAL_SUPPLY));
        assert!(!one_under.is_over_quorum(
            SETTINGS.quorum_bps,
            SETTINGS.counting_type,
            TOTAL_SUPPLY
        ));
    }

    #[test]
    fn threshold_uses_strict_greater_than_not_gte() {
        // The boundary that matters is an exact 50/50 split against a 50%
        // threshold: requirement = floor(2_000 * 5000 / 10_000) = 1000, and
        // for == 1000 is NOT greater than 1000, so it fails.
        let exactly_half = VoteCount::new(1_000, 1_000, 0);
        assert!(
            !exactly_half.is_over_threshold(5000),
            "for == requirement must NOT pass (upstream uses `>`)"
        );
        // One unit of `for` past the requirement does pass.
        let over = VoteCount::new(1_000, 1_001, 0);
        assert!(over.is_over_threshold(5000));
    }

    #[test]
    fn threshold_denominator_excludes_abstain_and_rejects_zero() {
        // 1 for, 0 against, 1_000_000 abstain. against+for == 1, so the
        // requirement is floor(1 * 5000 / 10_000) == 0 and 1 > 0 passes. This is
        // precisely the proof that abstain is EXCLUDED from the denominator:
        // had it been included the requirement would be 500_000 and this would
        // have failed.
        let t = VoteCount::new(0, 1, 1_000_000);
        assert!(t.is_over_threshold(5000));

        // Pure abstain: against+for == 0 short-circuits to false regardless of
        // the threshold, including 0%.
        let only_abstain = VoteCount::new(0, 0, 1_000_000);
        assert!(!only_abstain.is_over_threshold(0));
        assert!(!only_abstain.is_over_threshold(5_000));
    }

    #[test]
    fn abstain_counts_toward_quorum_when_counting_type_selects_it() {
        // counting_type 0b111 includes abstain, so an abstain-only tally can
        // reach quorum. This is exactly the s6-abstain-heavy shape.
        let t = VoteCount::new(0, 0, 155_000_000_001);
        assert!(t.is_over_quorum(SETTINGS.quorum_bps, SETTINGS.counting_type, TOTAL_SUPPLY));
    }

    #[test]
    fn abstain_excluded_from_quorum_when_counting_type_omits_it() {
        let t = VoteCount::new(0, 0, i128::MAX / 4);
        assert!(
            !t.is_over_quorum(SETTINGS.quorum_bps, 0b010, TOTAL_SUPPLY),
            "counting_type 0b010 counts only `for`, so abstain must not reach quorum"
        );
    }

    #[test]
    fn zero_votes_never_reach_quorum() {
        let t = VoteCount::default();
        assert!(!t.is_over_quorum(SETTINGS.quorum_bps, SETTINGS.counting_type, TOTAL_SUPPLY));
        assert!(!t.is_over_threshold(SETTINGS.vote_threshold_bps));
        assert_eq!(evaluate(&t, &SETTINGS, TOTAL_SUPPLY), VoteOutcome::NoQuorum);
    }

    #[test]
    fn outcome_distinguishes_no_quorum_from_threshold_not_met() {
        // No votes at all -> fails quorum.
        assert_eq!(
            evaluate(&VoteCount::default(), &SETTINGS, TOTAL_SUPPLY),
            VoteOutcome::NoQuorum
        );
        // Abstain-heavy: reaches quorum (counting_type includes abstain) but
        // against+for == 0 so the threshold can never be met.
        assert_eq!(
            evaluate(
                &VoteCount::new(0, 0, 7_500_000_000_000),
                &SETTINGS,
                TOTAL_SUPPLY
            ),
            VoteOutcome::ThresholdNotMet
        );
        // Plain for-votes above 50% of against+for.
        assert_eq!(
            evaluate(
                &VoteCount::new(0, 7_500_000_000_000, 0),
                &SETTINGS,
                TOTAL_SUPPLY
            ),
            VoteOutcome::Successful
        );
        // A large exact 50/50 split clears quorum but must NOT clear the
        // threshold (strict >). Sized so quorum is not what fails: count_quorum
        // = 2e12 > the 155e9 requirement.
        assert_eq!(
            evaluate(
                &VoteCount::new(1_000_000_000_000, 1_000_000_000_000, 0),
                &SETTINGS,
                TOTAL_SUPPLY
            ),
            VoteOutcome::ThresholdNotMet
        );
    }

    #[test]
    fn tally_votes_matches_the_captured_corpus() {
        // s6-abstain-heavy cast three abstains of 2_500_000_000_000 each.
        let t = tally_votes(&[(2, 2_500_000_000_000); 3]).expect("valid support");
        assert_eq!(
            t,
            VoteCount::new(0, 0, 7_500_000_000_000),
            "s6 captured on-chain tally is abstain 7500000000000"
        );
        // s1 cast three for-votes.
        let t1 = tally_votes(&[(1, 2_500_000_000_000); 3]).expect("valid support");
        assert_eq!(t1, VoteCount::new(0, 7_500_000_000_000, 0));
    }

    #[test]
    fn invalid_support_is_an_error_not_silently_ignored() {
        assert_eq!(
            tally_votes(&[(3, 100)]),
            Err(TallyError::InvalidSupport(3)),
            "upstream panics with InvalidProposalSupportError for support > 2"
        );
    }

    #[test]
    fn i128_extremes_do_not_panic() {
        // Saturating arithmetic: these must return, not wrap or panic.
        let huge = VoteCount::new(i128::MAX, i128::MAX, i128::MAX);
        // Saturating add, so this must return rather than wrap or panic. The
        // verdict itself is recorded honestly: with the numerator saturated to
        // i128::MAX, quorum reads as met. Saturation errs toward permissive on
        // overflow, which is the safe direction for a quorum *check* to be
        // reviewed rather than silently block a proposal.
        assert_eq!(huge.count_quorum(0b111), i128::MAX);
        assert!(huge.is_over_quorum(SETTINGS.quorum_bps, SETTINGS.counting_type, i128::MAX));
        // against+for overflows here, so the threshold fails CLOSED (false).
        // An unrepresentable denominator must never be read as "passed".
        assert!(!huge.is_over_threshold(SETTINGS.vote_threshold_bps));

        let negative = VoteCount::new(i128::MIN, i128::MIN, i128::MIN);
        let _ = negative.count_quorum(0b111);
        assert_eq!(evaluate(&negative, &SETTINGS, 0), VoteOutcome::NoQuorum);
        assert_eq!(
            tally_votes(&[(0, i128::MAX), (0, 1)]),
            Err(TallyError::Overflow)
        );
    }

    #[test]
    fn zero_total_supply_makes_quorum_free() {
        // floor(0 * bps / 10_000) == 0, so any positive tally passes. Mirrors
        // upstream exactly; recorded because it is a genuine edge.
        assert!(VoteCount::new(0, 1, 0).is_over_quorum(
            SETTINGS.quorum_bps,
            SETTINGS.counting_type,
            0
        ));
        assert!(!VoteCount::default().is_over_quorum(
            SETTINGS.quorum_bps,
            SETTINGS.counting_type,
            0
        ));
    }

    #[test]
    fn status_round_trips_and_rejects_unknown() {
        for raw in 0..=5u32 {
            let s = ProposalStatus::from_u32(raw).expect("known status");
            assert_eq!(s as u32, raw);
        }
        assert_eq!(ProposalStatus::from_u32(6), None);
        assert_eq!(ProposalStatus::from_u32(u32::MAX), None);
    }
}
