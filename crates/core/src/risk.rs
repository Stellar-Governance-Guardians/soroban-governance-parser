//! Rule-based risk classification for decoded governance actions.
//!
//! Rule 10 of the project charter: metrics and classifications are DATA, not
//! verdicts. Every risk tier is produced by an explicit, published rule list —
//! no opaque composite scores. Consumers see the matched rule for each tier.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    /// Rules that can move value or change contract code/ownership.
    Critical,
    /// Rules that change governance parameters or state.
    High,
    /// Rules with observable but bounded effects.
    Medium,
    /// Read-only or informational actions.
    Low,
    /// Function name matched no published rule — never silently downgraded.
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskClassification {
    pub tier: RiskTier,
    /// Exact rule that matched, so the classification is auditable.
    pub matched_rule: String,
    /// True only when the classification rests on a verified function name
    /// (from contractspecv0). Unverified decodings stay `Unverified`.
    pub verified: bool,
}

/// Published rule table. Matching is exact, case-sensitive, on the verified
/// function name. Extending this table is a spec change, not a code tweak.
pub static RULES: &[(&str, RiskTier, &str)] = &[
    // --- critical: value movement, code and ownership changes ---
    ("transfer", RiskTier::Critical, "moves token value"),
    (
        "transfer_from",
        RiskTier::Critical,
        "moves token value on behalf of another account",
    ),
    ("mint", RiskTier::Critical, "creates new token supply"),
    ("burn", RiskTier::Critical, "destroys token supply"),
    ("upgrade", RiskTier::Critical, "replaces contract WASM"),
    (
        "update_wasm_hash",
        RiskTier::Critical,
        "replaces contract WASM",
    ),
    (
        "set_admin",
        RiskTier::Critical,
        "changes contract administrator",
    ),
    (
        "change_admin",
        RiskTier::Critical,
        "changes contract administrator",
    ),
    (
        "transfer_ownership",
        RiskTier::Critical,
        "changes contract owner",
    ),
    (
        "renounce_ownership",
        RiskTier::Critical,
        "removes contract owner",
    ),
    (
        "execute",
        RiskTier::Critical,
        "executes a queued governance payload; effect depends on payload",
    ),
    // --- high: governance parameter changes ---
    (
        "queue",
        RiskTier::High,
        "schedules a passed proposal for execution",
    ),
    ("cancel", RiskTier::High, "cancels a proposal"),
    (
        "set_voting_delay",
        RiskTier::High,
        "changes governance timing",
    ),
    (
        "set_voting_period",
        RiskTier::High,
        "changes governance timing",
    ),
    ("set_quorum", RiskTier::High, "changes quorum requirement"),
    (
        "set_proposal_threshold",
        RiskTier::High,
        "changes who may propose",
    ),
    (
        "update_config",
        RiskTier::High,
        "changes contract configuration",
    ),
    (
        "set_config",
        RiskTier::High,
        "changes contract configuration",
    ),
    // --- medium: stateful but bounded ---
    (
        "propose",
        RiskTier::Medium,
        "creates a proposal; effect deferred to execution",
    ),
    (
        "delegate",
        RiskTier::Medium,
        "moves voting power between accounts",
    ),
    ("vote", RiskTier::Medium, "casts a vote"),
    ("cast_vote", RiskTier::Medium, "casts a vote"),
    ("approve", RiskTier::Medium, "grants a token allowance"),
    // --- low: read-only / informational ---
    ("get_proposal", RiskTier::Low, "read-only accessor"),
    ("get_votes", RiskTier::Low, "read-only accessor"),
    ("balance", RiskTier::Low, "read-only accessor"),
    ("decimals", RiskTier::Low, "read-only accessor"),
    ("name", RiskTier::Low, "read-only accessor"),
    ("symbol", RiskTier::Low, "read-only accessor"),
    ("version", RiskTier::Low, "read-only accessor"),
];

/// Classify a function name that was verified against contractspecv0.
pub fn classify_verified(function_name: &str) -> RiskClassification {
    for (rule, tier, _why) in RULES {
        if *rule == function_name {
            return RiskClassification {
                tier: *tier,
                matched_rule: (*rule).to_owned(),
                verified: true,
            };
        }
    }
    RiskClassification {
        tier: RiskTier::Unverified,
        matched_rule: "no published rule matched this function name".to_owned(),
        verified: true,
    }
}

/// Classify when the function name could NOT be verified (e.g. missing
/// contractspecv0). Fail-closed: always `Unverified`, never a downgrade.
pub fn classify_unverified(function_name: &str) -> RiskClassification {
    RiskClassification {
        tier: RiskTier::Unverified,
        matched_rule: format!(
            "function name `{function_name}` not verified against a contract spec"
        ),
        verified: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_rules_match() {
        assert_eq!(classify_verified("transfer").tier, RiskTier::Critical);
        assert_eq!(classify_verified("propose").tier, RiskTier::Medium);
        assert_eq!(classify_verified("get_votes").tier, RiskTier::Low);
    }

    #[test]
    fn unknown_function_is_unverified_not_low() {
        let c = classify_verified("sweep_everything");
        assert_eq!(c.tier, RiskTier::Unverified);
        assert!(c.verified);
    }

    #[test]
    fn unverified_name_never_downgrades() {
        // Even a name that WOULD match a rule stays Unverified if the name
        // itself was not verified against a spec.
        let c = classify_unverified("transfer");
        assert_eq!(c.tier, RiskTier::Unverified);
        assert!(!c.verified);
    }

    #[test]
    fn prefix_attack_does_not_match() {
        // "transfer_all_funds" must not inherit "transfer"'s tier silently.
        assert_eq!(
            classify_verified("transfer_all_funds").tier,
            RiskTier::Unverified
        );
    }
}
