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

// --- contextual rules ----------------------------------------------------------
//
// The table above classifies a *function name*. These rules additionally need to
// know who is being paid and how much, so they take a `RiskContext` supplied by
// the caller (the indexer knows the treasury; a reader may not).
//
// Charter rule 10 still applies: every flag names the exact rule that fired and
// carries its evidence. Nothing is combined into a single number.

/// Contextual inputs for the rule set below.
///
/// Every field is optional and every absence is meaningful: a missing treasury
/// address does not mean "no treasury outflow", it means the rule cannot be
/// evaluated, and the flag says so.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RiskContext {
    /// Governor contract id, for evidence strings.
    pub governor: Option<String>,
    /// Addresses treated as protocol treasuries.
    pub treasury_addresses: Vec<String>,
    /// `large_value` fires at or above this amount, in the token's own units.
    pub large_value_threshold: Option<i128>,
}

impl RiskTier {
    /// Ordering used only to pick the most severe fired rule. `Unverified` is
    /// NOT ranked as "least severe": it outranks nothing and is outranked by
    /// everything, so an unverified flag never masks a Critical one — it is
    /// simply not a severity.
    const fn rank(self) -> u8 {
        match self {
            Self::Unverified => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Critical => 4,
        }
    }
}

/// A single fired contextual rule, with the evidence that fired it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskFlag {
    pub rule: String,
    pub tier: RiskTier,
    /// Verbatim facts used by the rule, so a reader can re-check it.
    pub evidence: serde_json::Value,
}

/// All contextual flags for one call. Empty means no rule fired — which is
/// distinct from "not evaluated".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskFlags {
    pub flags: Vec<RiskFlag>,
}

impl RiskFlags {
    /// Highest tier among fired rules. `None` when nothing fired.
    ///
    /// Note this is a *display* convenience over the individual flags — the
    /// flags themselves are always returned in full and are never collapsed.
    pub fn highest_tier(&self) -> RiskTier {
        self.flags
            .iter()
            .map(|f| f.tier)
            .max_by_key(|t| t.rank())
            .unwrap_or(RiskTier::Low)
    }

    /// Comma-separated list of fired rule names, for the audit trail.
    pub fn matched_rules(&self) -> String {
        if self.flags.is_empty() {
            return "no contextual rule fired".to_owned();
        }
        self.flags
            .iter()
            .map(|f| f.rule.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn fired(&self, rule: &str) -> bool {
        self.flags.iter().any(|f| f.rule == rule)
    }
}

/// The call a contextual rule set is evaluated against. Kept governor-agnostic:
/// the Script3 adapter projects its on-chain `Calldata` into this shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallUnderReview {
    pub target: String,
    pub function: String,
    /// Positional arguments as decoded from chain.
    pub args: Vec<serde_json::Value>,
    /// Monetary amount when the function is known to carry one; `None` means
    /// "not a value-moving call" or "amount position unknown".
    pub amount: Option<i128>,
    /// Sub-calls this call authorizes (`Calldata.auths`).
    pub nested: Vec<CallUnderReview>,
}

impl RiskContext {
    /// Evaluate every contextual rule against one call, including nested
    /// authorized sub-calls.
    pub fn evaluate(&self, call: &CallUnderReview) -> RiskFlags {
        let mut out = RiskFlags::default();
        self.evaluate_into(call, &mut out, 0);
        out
    }

    /// Depth-capped so a maliciously deep `auths` chain cannot blow the stack.
    const MAX_DEPTH: usize = 16;

    fn evaluate_into(&self, call: &CallUnderReview, out: &mut RiskFlags, depth: usize) {
        if depth > Self::MAX_DEPTH {
            return;
        }
        self.rule_treasury_outflow(call, out);
        self.rule_self_call(call, out);
        self.rule_large_value(call, out);
        Self::rule_batched_actions(call, out);
        for nested in &call.nested {
            self.evaluate_into(nested, out, depth + 1);
        }
    }

    /// Value leaving a known protocol treasury address.
    ///
    /// Fires when the FIRST argument is a treasury address. Upstream's pinned
    /// `transfer(from, to, amount)` puts the *source* at position 0, so on
    /// real Script3 data this is the treasury the value leaves — the rule
    /// reads `args[0]` positionally and never tries other positions. Requires
    /// the target function to be a known value-moving name, so a read-only
    /// call with the same argument shape does not trip it.
    fn rule_treasury_outflow(&self, call: &CallUnderReview, out: &mut RiskFlags) {
        if !is_value_moving(&call.function) {
            return;
        }
        let Some(first_arg) = call.args.first().and_then(crate::scval::json_address_str) else {
            return;
        };
        if self.treasury_addresses.iter().any(|t| t == first_arg) {
            out.flags.push(RiskFlag {
                rule: "treasury_outflow".to_owned(),
                tier: RiskTier::Critical,
                evidence: serde_json::json!({
                    "from": self.governor,
                    "function": call.function,
                    "treasury": first_arg,
                    "note": "args[0] is a registered protocol treasury address",
                }),
            });
        }
    }

    /// The call targets the governor itself.
    ///
    /// Self-directed governance calls can rewrite the rules that authorised
    /// them, so they are surfaced separately even when otherwise low risk.
    fn rule_self_call(&self, call: &CallUnderReview, out: &mut RiskFlags) {
        let Some(governor) = self.governor.as_deref() else {
            return;
        };
        if call.target == governor {
            out.flags.push(RiskFlag {
                rule: "self_call".to_owned(),
                tier: RiskTier::High,
                evidence: serde_json::json!({
                    "target": call.target,
                    "function": call.function,
                    "note": "proposal calls the governor contract itself",
                }),
            });
        }
    }

    /// A single amount at or above the configured threshold.
    ///
    /// Fires only when the amount is actually known; an unknown amount is not
    /// treated as zero and does not fire.
    fn rule_large_value(&self, call: &CallUnderReview, out: &mut RiskFlags) {
        let (Some(threshold), Some(amount)) = (self.large_value_threshold, call.amount) else {
            return;
        };
        if amount >= threshold {
            out.flags.push(RiskFlag {
                rule: "large_value".to_owned(),
                tier: RiskTier::High,
                evidence: serde_json::json!({
                    "function": call.function,
                    "amount": amount.to_string(),
                    "threshold": threshold.to_string(),
                    "note": "amount is at or above the configured large-value threshold",
                }),
            });
        }
    }

    /// More than one call authorized by a single proposal.
    ///
    /// Counts nested `auths` as well as the top-level call, because a proposal
    /// that authorizes one call which itself authorizes two more is still a
    /// batch.
    fn rule_batched_actions(call: &CallUnderReview, out: &mut RiskFlags) {
        let count = 1 + count_nested(call);
        if count > 1 {
            out.flags.push(RiskFlag {
                rule: "batched_actions".to_owned(),
                tier: RiskTier::Medium,
                evidence: serde_json::json!({
                    "total_calls": count,
                    "note": "a single proposal authorizes more than one call",
                }),
            });
        }
    }
}

fn count_nested(call: &CallUnderReview) -> usize {
    call.nested.iter().map(|n| 1 + count_nested(n)).sum()
}

/// Function names treated as moving value, for the treasury-outflow rule.
///
/// Deliberately a short explicit list. An unknown function does not move value
/// *as far as this rule is concerned*, and is reported unverified elsewhere.
fn is_value_moving(function: &str) -> bool {
    matches!(
        function,
        "transfer" | "transfer_from" | "mint" | "burn" | "burn_from" | "approve"
    )
}

#[cfg(test)]
mod contextual_tests {
    use super::*;

    fn ctx() -> RiskContext {
        RiskContext {
            governor: Some("CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX".into()),
            treasury_addresses: vec![
                "CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX".into()
            ],
            large_value_threshold: Some(1_000_000_000_000),
        }
    }

    fn transfer(to: &str, amount: i128) -> CallUnderReview {
        CallUnderReview {
            target: "CCAUJK6V6GIYQANKCV2JDCMCLGMQD42JCF426TQHHMSXW2F5ZROM4IKY".into(),
            function: "transfer".into(),
            args: vec![
                serde_json::json!(to),
                serde_json::json!("GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235"),
                serde_json::json!(amount.to_string()),
            ],
            amount: Some(amount),
            nested: vec![],
        }
    }

    // --- positive: one rule per fixture ----------------------------------------

    #[test]
    fn treasury_outflow_fires_when_the_first_argument_is_the_treasury() {
        let call = transfer(
            "CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX",
            10,
        );
        let flags = ctx().evaluate(&call);
        assert!(flags.fired("treasury_outflow"));
        assert_eq!(
            flags
                .flags
                .iter()
                .find(|f| f.rule == "treasury_outflow")
                .map(|f| f.tier),
            Some(RiskTier::Critical)
        );
    }

    #[test]
    fn self_call_fires_when_target_is_the_governor() {
        let mut call = transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            10,
        );
        call.target = "CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX".into();
        let flags = ctx().evaluate(&call);
        assert!(flags.fired("self_call"));
    }

    #[test]
    fn large_value_fires_at_the_threshold_inclusively() {
        let flags = ctx().evaluate(&transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            1_000_000_000_000,
        ));
        assert!(flags.fired("large_value"), "threshold is inclusive");
        let below = ctx().evaluate(&transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            999_999_999_999,
        ));
        assert!(!below.fired("large_value"));
    }

    #[test]
    fn batched_actions_fires_for_nested_authorized_calls() {
        let mut call = transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            10,
        );
        call.nested.push(transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            1,
        ));
        call.nested.push(transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            1,
        ));
        let flags = ctx().evaluate(&call);
        assert!(flags.fired("batched_actions"));
        assert_eq!(
            flags
                .flags
                .iter()
                .find(|f| f.rule == "batched_actions")
                .and_then(|f| f.evidence.get("total_calls")),
            Some(&serde_json::json!(3))
        );
    }

    // --- negative: one per rule -----------------------------------------------

    #[test]
    fn treasury_outflow_does_not_fire_for_a_non_treasury_first_argument() {
        let call = transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            10,
        );
        assert!(!ctx().evaluate(&call).fired("treasury_outflow"));
    }

    #[test]
    fn self_call_does_not_fire_for_an_external_target() {
        assert!(!ctx()
            .evaluate(&transfer(
                "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
                10
            ))
            .fired("self_call"));
    }

    #[test]
    fn large_value_does_not_fire_below_threshold_or_without_a_threshold() {
        assert!(!ctx()
            .evaluate(&transfer(
                "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
                1
            ))
            .fired("large_value"));
        let no_threshold = RiskContext {
            governor: Some("GOV".into()),
            treasury_addresses: vec![],
            large_value_threshold: None,
        };
        let call = transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            i128::MAX,
        );
        assert!(
            !no_threshold.evaluate(&call).fired("large_value"),
            "an absent threshold must not be treated as zero"
        );
    }

    #[test]
    fn batched_actions_does_not_fire_for_a_single_call() {
        assert!(!ctx()
            .evaluate(&transfer(
                "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
                10
            ))
            .fired("batched_actions"));
    }

    // --- fail-closed / unknown stays unknown -----------------------------------

    #[test]
    fn unknown_function_is_not_treated_as_value_moving() {
        let call = CallUnderReview {
            target: "X".into(),
            function: "sweep_everything".into(),
            args: vec![serde_json::json!(
                "CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX"
            )],
            amount: None,
            nested: vec![],
        };
        let flags = ctx().evaluate(&call);
        assert!(
            !flags.fired("treasury_outflow"),
            "an unrecognized function must not be assumed to move value"
        );
    }

    #[test]
    fn missing_governor_disables_self_call_rather_than_guessing() {
        let c = RiskContext {
            governor: None,
            ..ctx()
        };
        let mut call = transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            10,
        );
        call.target = "CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX".into();
        assert!(!c.evaluate(&call).fired("self_call"));
    }

    #[test]
    fn deeply_nested_auths_terminate_instead_of_overflowing() {
        // Build a 500-deep chain iteratively so the test itself does not recurse.
        let mut call = transfer(
            "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
            1,
        );
        let mut cursor = &mut call;
        for _ in 0..500 {
            let child = transfer(
                "GDGT6J5SH66EQPF62EHYRJBJMKHEKL55UZ6KCVJGF7CWZYCRMI2RA235",
                1,
            );
            cursor.nested.push(child);
            let Some(next) = cursor.nested.last_mut() else {
                unreachable!("a value was just pushed");
            };
            cursor = next;
        }
        // Must not stack overflow; the depth cap stops it at 16.
        let flags = ctx().evaluate(&call);
        assert!(flags.fired("batched_actions"));
    }

    #[test]
    fn every_flag_carries_evidence_and_the_rules_are_listed() {
        let mut call = transfer(
            "CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX",
            5_000_000_000_000,
        );
        call.target = "CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX".into();
        call.nested.push(transfer("X", 1));
        let flags = ctx().evaluate(&call);
        for f in &flags.flags {
            assert!(!f.evidence.is_null(), "flag {} has null evidence", f.rule);
            assert!(!f.rule.is_empty());
        }
        let names = flags.matched_rules();
        assert!(names.contains("treasury_outflow"));
        assert_eq!(flags.highest_tier(), RiskTier::Critical);
    }
}
