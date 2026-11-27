//! OpenZeppelin `stellar-contracts` Governor adapter.
//!
//! Every shape encoded here is transcribed from the **pinned** upstream source
//! and from the contract's own committed `contractspecv0` section, never
//! guessed:
//!
//! ```text
//! github.com/OpenZeppelin/stellar-contracts @ b40c5eaefe6a29f0030f00bd2d730b7a91cce330
//!   packages/governance/src/governor/mod.rs   — ProposalState, all #[contractevent] structs
//!   packages/governance/src/votes/            — checkpointed voting power
//! ```
//!
//! `#[contractevent]` defaults matter and are reproduced exactly:
//! `topic[0]` is the struct name in `snake_case`, every `#[topic]` field follows
//! in declaration order, and the data is a **symbol-keyed map** of the
//! remaining fields (the macro's default `data_format = "map"`). Script3 uses a
//! positional vector instead, so the two adapters are deliberately separate.
//!
//! # What is verified, and how
//!
//! - **Proposal state** is differentially tested against the committed
//!   seed-v2 `simulateTransaction` captures of `proposal_state` for proposals
//!   `o1` through `o6` (see `crates/core/tests/openzeppelin_differential.rs`).
//! - **Event names and layouts** are read from the pinned source and checked
//!   against the deployed governor's embedded spec in the unit tests below.
//!   No `getEvents` capture for this governor exists yet, so event decoding is
//!   *source-verified, not capture-verified*. The adapter never claims more:
//!   undocumented events fall through to `unverified` rather than being
//!   approximated.
//!
//! # Fail-closed
//!
//! A topic arity mismatch, a non-map data payload, a missing field, or an
//! unknown `ProposalState` discriminant all produce `Err`. Nothing is
//! defaulted, and no field is invented.

use stellar_xdr::{ScMapEntry, ScVal};

use crate::adapter::{GovernorAdapter, RawCall, RawEvent};
use crate::error::{AdapterError, DecodeError};
use crate::risk::{classify_unverified, classify_verified, CallUnderReview, RiskContext};
use crate::types::{DecodedCall, DecodedProposal, DecodingStatus, NormalizedEvent};

/// Stable adapter identifier.
pub const ADAPTER_NAME: &str = "openzeppelin-stellar-governor";

/// The pinned OpenZeppelin governor deployed and seeded on testnet, registered
/// in `deployments.json` under `contracts.testnet.seedV2OpenZeppelin.ozGovernor`.
pub const GOVERNOR_CONTRACT_ID: &str = "CAONSV2R2YCNJS6DSN366M5LXLHWT46ES57MD6KOM725YFQSGRLHDCMM";

/// The votes/token contract the governor reads checkpointed power from.
pub const TOKEN_CONTRACT_ID: &str = "CA77UOHZIPLJKPRVQ7FOYRT5ATAFQYVDTVWRF4ZHNCN2OK274CCQ4V2R";

/// Event names emitted by the pinned governor, with their exact topic arity
/// *including* the leading snake_case event-name topic. `quorum_changed` has no
/// `#[topic]` fields, so its arity is 1.
pub const EVENT_ARITY: &[(&str, usize)] = &[
    ("proposal_created", 3),
    ("vote_cast", 3),
    ("proposal_queued", 2),
    ("proposal_executed", 2),
    ("proposal_cancelled", 2),
    ("quorum_changed", 1),
];

/// How voting power is derived for this governor: a delegated balance read at
/// the proposal's `vote_snapshot` checkpoint, never recomputed from the event
/// stream. Recorded as data so it can be displayed and asserted.
pub const VOTE_POWER_MODEL: &str = "checkpointed_oz_votes_contract";

/// Lifecycle state, mirroring the pinned `ProposalState` enum. The
/// discriminants differ from Script3's, so this is a separate type on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OzProposalStatus {
    Pending = 0,
    Active = 1,
    Defeated = 2,
    Canceled = 3,
    Succeeded = 4,
    Queued = 5,
    Expired = 6,
    Executed = 7,
}

impl OzProposalStatus {
    /// Strictly map an on-chain discriminant. Unknown values are `None`, never
    /// coerced to the nearest known state.
    pub fn from_u32(raw: u32) -> Option<Self> {
        Some(match raw {
            0 => Self::Pending,
            1 => Self::Active,
            2 => Self::Defeated,
            3 => Self::Canceled,
            4 => Self::Succeeded,
            5 => Self::Queued,
            6 => Self::Expired,
            7 => Self::Executed,
            _ => return None,
        })
    }
}

/// Governor state reconstructed from a chain read.
///
/// `proposal_state` only returns the enum discriminant; the proposal's
/// calldata, `vote_snapshot` and `vote_end` are carried by the
/// `proposal_created` event because this governor does not store calldata in
/// contract state. That split is a property of the upstream design, not a gap
/// in this adapter.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OzProposalState {
    pub governor: String,
    /// 32-byte proposal id as lowercase hex.
    pub proposal_id: String,
    pub status: OzProposalStatus,
}

// --- ScVal helpers -------------------------------------------------------------

fn decode_err(message: impl Into<String>) -> AdapterError {
    AdapterError::Decode {
        adapter: ADAPTER_NAME,
        source: DecodeError::UnsupportedScVal(message.into()),
    }
}

fn want_symbol(v: &ScVal) -> Result<String, AdapterError> {
    match v {
        ScVal::Symbol(s) => Ok(s.to_string()),
        other => Err(decode_err(format!("expected Symbol, got {other:?}"))),
    }
}

fn want_u32(v: &ScVal, field: &str) -> Result<u32, AdapterError> {
    match v {
        ScVal::U32(n) => Ok(*n),
        other => Err(decode_err(format!("{field} expected U32, got {other:?}"))),
    }
}

fn want_string(v: &ScVal, field: &str) -> Result<String, AdapterError> {
    match v {
        ScVal::String(s) => Ok(s.to_string()),
        other => Err(decode_err(format!(
            "{field} expected String, got {other:?}"
        ))),
    }
}

fn want_address(v: &ScVal, field: &str) -> Result<String, AdapterError> {
    match v {
        ScVal::Address(a) => Ok(a.to_string()),
        other => Err(decode_err(format!(
            "{field} expected Address, got {other:?}"
        ))),
    }
}

/// `BytesN<32>` proposals ids are `ScVal::Bytes` with exactly 32 bytes. Any
/// other length or variant is an error — an id is not a free-form string.
fn want_bytes32_hex(v: &ScVal, field: &str) -> Result<String, AdapterError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    match v {
        ScVal::Bytes(bytes) if bytes.len() == 32 => {
            let mut out = String::with_capacity(64);
            for byte in bytes.iter() {
                out.push(HEX[usize::from(byte >> 4)] as char);
                out.push(HEX[usize::from(byte & 0x0f)] as char);
            }
            Ok(out)
        }
        other => Err(decode_err(format!(
            "{field} expected 32-byte BytesN<32>, got {other:?}"
        ))),
    }
}

fn want_vec<'a>(v: &'a ScVal, field: &str) -> Result<&'a [ScVal], AdapterError> {
    match v {
        ScVal::Vec(Some(items)) => Ok(items.as_slice()),
        other => Err(decode_err(format!("{field} expected Vec, got {other:?}"))),
    }
}

fn scmap(v: &ScVal) -> Result<&[ScMapEntry], AdapterError> {
    match v {
        ScVal::Map(Some(entries)) => Ok(entries.as_slice()),
        other => Err(decode_err(format!("expected Map data, got {other:?}"))),
    }
}

/// Pull one symbol-keyed field out of an event data map. The `contractevent`
/// macro sorts map keys, so lookup is by key, never by position.
fn map_field<'a>(v: &'a ScVal, field: &str) -> Result<&'a ScVal, AdapterError> {
    scmap(v)?
        .iter()
        .find(|entry| matches!(&entry.key, ScVal::Symbol(s) if s.to_string() == field))
        .map(|entry| &entry.val)
        .ok_or_else(|| decode_err(format!("event data is missing required field `{field}`")))
}

/// `quorum_changed` has no topic fields and `proposal_executed`/`cancelled`
/// have no data fields. The macro emits an empty map for the latter; a void is
/// accepted too because it is the same absence of fields, not a guess.
fn expect_no_data(v: &ScVal) -> Result<(), AdapterError> {
    match v {
        ScVal::Void | ScVal::Map(None) => Ok(()),
        ScVal::Map(Some(entries)) if entries.is_empty() => Ok(()),
        other => Err(decode_err(format!(
            "event declares no data fields but got {other:?}"
        ))),
    }
}

fn u128_string(v: &ScVal, field: &str) -> Result<String, AdapterError> {
    match v {
        ScVal::U128(parts) => {
            Ok((((u128::from(parts.hi)) << 64) | u128::from(parts.lo)).to_string())
        }
        other => Err(decode_err(format!("{field} expected U128, got {other:?}"))),
    }
}

fn json_array(v: &ScVal, field: &str) -> Result<Vec<serde_json::Value>, AdapterError> {
    want_vec(v, field)?
        .iter()
        .map(|item| {
            crate::scval::scval_to_json(item).map_err(|source| AdapterError::Decode {
                adapter: ADAPTER_NAME,
                source,
            })
        })
        .collect()
}

// --- event normalization -------------------------------------------------------

fn normalize_proposal_created(
    event: &RawEvent<'_>,
    payload: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    payload.insert(
        "proposal_id".into(),
        want_bytes32_hex(&event.topics[1], "proposal_id")?.into(),
    );
    payload.insert(
        "proposer".into(),
        want_address(&event.topics[2], "proposer")?.into(),
    );
    let targets = want_vec(map_field(event.value, "targets")?, "targets")?
        .iter()
        .map(|item| want_address(item, "targets[]"))
        .collect::<Result<Vec<_>, _>>()?;
    payload.insert("targets".into(), serde_json::json!(targets));

    let functions = want_vec(map_field(event.value, "functions")?, "functions")?
        .iter()
        .map(want_symbol)
        .collect::<Result<Vec<_>, _>>()?;
    payload.insert("functions".into(), serde_json::json!(functions));

    let args = want_vec(map_field(event.value, "args")?, "args")?
        .iter()
        .map(|item| json_array(item, "args[]"))
        .collect::<Result<Vec<_>, _>>()?;
    payload.insert("args".into(), serde_json::json!(args));

    payload.insert(
        "vote_snapshot".into(),
        serde_json::json!(want_u32(
            map_field(event.value, "vote_snapshot")?,
            "vote_snapshot"
        )?),
    );
    payload.insert(
        "vote_end".into(),
        serde_json::json!(want_u32(map_field(event.value, "vote_end")?, "vote_end")?),
    );
    payload.insert(
        "description".into(),
        want_string(map_field(event.value, "description")?, "description")?.into(),
    );
    Ok(())
}

fn normalize_vote_cast(
    event: &RawEvent<'_>,
    payload: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), AdapterError> {
    // Topic order differs from Script3: voter, then proposal_id.
    payload.insert(
        "voter".into(),
        want_address(&event.topics[1], "voter")?.into(),
    );
    payload.insert(
        "proposal_id".into(),
        want_bytes32_hex(&event.topics[2], "proposal_id")?.into(),
    );
    let vote_type = want_u32(map_field(event.value, "vote_type")?, "vote_type")?;
    if vote_type > 2 {
        return Err(decode_err(format!(
            "vote_type must be 0 (against), 1 (for) or 2 (abstain); got {vote_type}"
        )));
    }
    payload.insert("vote_type".into(), serde_json::json!(vote_type));
    payload.insert(
        "weight".into(),
        u128_string(map_field(event.value, "weight")?, "weight")?.into(),
    );
    payload.insert(
        "reason".into(),
        want_string(map_field(event.value, "reason")?, "reason")?.into(),
    );
    Ok(())
}

/// Adapter for the pinned OpenZeppelin `stellar-contracts` governor.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenZeppelinAdapter;

impl GovernorAdapter for OpenZeppelinAdapter {
    fn name(&self) -> &'static str {
        ADAPTER_NAME
    }

    fn matches_event(&self, event: &RawEvent<'_>) -> bool {
        if event.contract_id != GOVERNOR_CONTRACT_ID {
            return false;
        }
        let Some(ScVal::Symbol(sym)) = event.topics.first() else {
            return false;
        };
        let name = sym.to_string();
        EVENT_ARITY
            .iter()
            .any(|(known, arity)| *known == name && event.topics.len() == *arity)
    }

    fn normalize_event(&self, event: &RawEvent<'_>) -> Result<NormalizedEvent, AdapterError> {
        let Some(ScVal::Symbol(sym)) = event.topics.first() else {
            return Err(AdapterError::TopicMismatch(ADAPTER_NAME));
        };
        let kind = sym.to_string();
        let mut payload = serde_json::Map::new();
        match kind.as_str() {
            "proposal_created" => normalize_proposal_created(event, &mut payload)?,
            "vote_cast" => normalize_vote_cast(event, &mut payload)?,
            "proposal_queued" => {
                payload.insert(
                    "proposal_id".into(),
                    want_bytes32_hex(&event.topics[1], "proposal_id")?.into(),
                );
                payload.insert(
                    "eta".into(),
                    serde_json::json!(want_u32(map_field(event.value, "eta")?, "eta")?),
                );
            }
            "proposal_executed" | "proposal_cancelled" => {
                payload.insert(
                    "proposal_id".into(),
                    want_bytes32_hex(&event.topics[1], "proposal_id")?.into(),
                );
                expect_no_data(event.value)?;
            }
            "quorum_changed" => {
                payload.insert(
                    "old_quorum".into(),
                    u128_string(map_field(event.value, "old_quorum")?, "old_quorum")?.into(),
                );
                payload.insert(
                    "new_quorum".into(),
                    u128_string(map_field(event.value, "new_quorum")?, "new_quorum")?.into(),
                );
            }
            other => {
                return Err(decode_err(format!(
                    "unhandled OpenZeppelin governor event `{other}`"
                )))
            }
        }

        let topics = event
            .topics
            .iter()
            .map(|t| {
                crate::scval::scval_to_json(t).map_err(|source| AdapterError::Decode {
                    adapter: ADAPTER_NAME,
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .map(serde_json::Value::to_string)
            .collect();

        Ok(NormalizedEvent {
            governor: ADAPTER_NAME.to_owned(),
            topics,
            ledger: event.ledger,
            tx_hash: event.tx_hash.to_owned(),
            contract_id: event.contract_id.to_owned(),
            payload: serde_json::Value::Object(payload),
            decoding: DecodingStatus::Verified,
        })
    }

    fn decode_call(
        &self,
        call: &RawCall<'_>,
        spec: Option<&crate::spec::ContractSpec>,
    ) -> Result<DecodedCall, AdapterError> {
        let (arguments, decoding) = match spec {
            Some(s) => match crate::spec::decode_args_with_spec(s, call.function_name, call.args) {
                Ok(named) => (serde_json::json!(named), DecodingStatus::Verified),
                Err(_) => (positional(call)?, DecodingStatus::Unverified),
            },
            None => (positional(call)?, DecodingStatus::Unverified),
        };
        let risk = if decoding == DecodingStatus::Verified {
            classify_verified(call.function_name)
        } else {
            classify_unverified(call.function_name)
        };
        Ok(DecodedCall {
            contract_id: call.contract_id.to_owned(),
            function_name: call.function_name.to_owned(),
            arguments,
            decoding,
            risk,
        })
    }
}

fn positional(call: &RawCall<'_>) -> Result<serde_json::Value, AdapterError> {
    let args = call
        .args
        .iter()
        .map(crate::scval::scval_to_json)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source,
        })?;
    Ok(serde_json::json!(args))
}

impl OpenZeppelinAdapter {
    /// The stated voting-power model for this governor.
    pub const fn vote_power_model(&self) -> &'static str {
        VOTE_POWER_MODEL
    }

    /// Decode the `proposal_state` read (`ProposalState` as a `u32`).
    ///
    /// Differentially tested against the committed captures for `o2`, `o4`
    /// and `o6`. An unknown discriminant is an error, never a default.
    pub fn state_from_chain(
        &self,
        proposal_id_hex: &str,
        proposal_state: &ScVal,
    ) -> Result<OzProposalState, AdapterError> {
        if proposal_id_hex.len() != 64 || !proposal_id_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(decode_err(format!(
                "proposal_id must be 64 lowercase-or-uppercase hex characters, got `{proposal_id_hex}`"
            )));
        }
        let raw = want_u32(proposal_state, "proposal_state")?;
        let status = OzProposalStatus::from_u32(raw).ok_or_else(|| {
            decode_err(format!(
                "unknown OpenZeppelin ProposalState discriminant {raw}"
            ))
        })?;
        Ok(OzProposalState {
            governor: ADAPTER_NAME.to_owned(),
            proposal_id: proposal_id_hex.to_ascii_lowercase(),
            status,
        })
    }

    /// Project a normalized `proposal_created` payload into the shared
    /// `DecodedProposal` shape, applying the published risk rules to every
    /// target call.
    ///
    /// The payload is what [`GovernorAdapter::normalize_event`] produced from
    /// the event's own data: the governor does not store calldata in contract
    /// state, so the creation event is the only authoritative source. Target
    /// argument *names* remain unverified — no target spec is supplied here —
    /// but each call's function name is chain-verified and drives the same
    /// published rule table as Script3.
    pub fn proposal_from_created(
        &self,
        payload: &serde_json::Value,
        context: &RiskContext,
    ) -> Result<DecodedProposal, AdapterError> {
        let object = payload
            .as_object()
            .ok_or_else(|| decode_err("proposal_created payload must be a JSON object"))?;
        let proposal_id = object
            .get("proposal_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| decode_err("proposal_created payload is missing `proposal_id`"))?;
        let proposer = object
            .get("proposer")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let targets = object
            .get("targets")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| decode_err("proposal_created payload is missing `targets`"))?;
        let functions = object
            .get("functions")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| decode_err("proposal_created payload is missing `functions`"))?;
        let args = object
            .get("args")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| decode_err("proposal_created payload is missing `args`"))?;
        if targets.len() != functions.len() || targets.len() != args.len() {
            return Err(decode_err(format!(
                "proposal action vectors disagree: {} targets, {} functions, {} args",
                targets.len(),
                functions.len(),
                args.len()
            )));
        }
        if targets.is_empty() {
            return Err(decode_err(
                "proposal has no target calls; refusing to emit an empty decoding",
            ));
        }

        let mut calls = Vec::with_capacity(targets.len());
        for ((target, function), call_args) in targets.iter().zip(functions).zip(args) {
            let target = target
                .as_str()
                .ok_or_else(|| decode_err("targets[] must contain address strings"))?;
            let function = function
                .as_str()
                .ok_or_else(|| decode_err("functions[] must contain symbol strings"))?;
            let call_args = call_args
                .as_array()
                .ok_or_else(|| decode_err("args[] must contain argument arrays"))?;
            let flags = context.evaluate(&CallUnderReview {
                target: target.to_owned(),
                function: function.to_owned(),
                args: call_args.clone(),
                amount: None,
                nested: Vec::new(),
            });
            calls.push(DecodedCall {
                contract_id: target.to_owned(),
                function_name: function.to_owned(),
                arguments: serde_json::Value::Array(call_args.clone()),
                // Argument names need the target contract's spec, which this
                // event does not carry. Positional and explicitly unverified.
                decoding: DecodingStatus::Unverified,
                risk: crate::risk::RiskClassification {
                    tier: flags.highest_tier(),
                    matched_rule: flags.matched_rules(),
                    verified: false,
                },
            });
        }

        Ok(DecodedProposal {
            governor: ADAPTER_NAME.to_owned(),
            proposal_id: proposal_id.to_owned(),
            proposer,
            calls,
            // The proposal structure and function names came from the
            // governor's own event; individual argument names did not.
            decoding: DecodingStatus::Verified,
        })
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use stellar_xdr::{
        BytesM, ContractId, Hash, ScAddress, ScBytes, ScMap, ScString, ScSymbol, ScVec, StringM,
        VecM,
    };

    fn sym(value: &str) -> ScVal {
        ScVal::Symbol(ScSymbol(StringM::try_from(value).expect("symbol fits")))
    }

    fn map(entries: Vec<(&str, ScVal)>) -> ScVal {
        ScVal::Map(Some(ScMap(
            entries
                .into_iter()
                .map(|(key, val)| ScMapEntry { key: sym(key), val })
                .collect::<Vec<_>>()
                .try_into()
                .expect("map fits"),
        )))
    }

    fn vec(items: Vec<ScVal>) -> ScVal {
        ScVal::Vec(Some(ScVec(VecM::try_from(items).expect("vec fits"))))
    }

    fn contract_address(byte: u8) -> ScVal {
        ScVal::Address(ScAddress::Contract(ContractId(Hash([byte; 32]))))
    }

    fn proposal_id_bytes(byte: u8) -> ScVal {
        ScVal::Bytes(ScBytes(BytesM::try_from(vec![byte; 32]).expect("32 bytes")))
    }

    fn u128_val(value: u128) -> ScVal {
        ScVal::U128(stellar_xdr::UInt128Parts {
            hi: (value >> 64) as u64,
            lo: value as u64,
        })
    }

    fn string(value: &str) -> ScVal {
        ScVal::String(ScString(StringM::try_from(value).expect("string fits")))
    }

    fn created_event() -> (Vec<ScVal>, ScVal) {
        (
            vec![
                sym("proposal_created"),
                proposal_id_bytes(0x11),
                contract_address(0xaa),
            ],
            map(vec![
                ("targets", vec(vec![contract_address(0x22)])),
                ("functions", vec(vec![sym("transfer")])),
                (
                    "args",
                    vec(vec![vec(vec![contract_address(0x33), u128_val(42)])]),
                ),
                ("vote_snapshot", ScVal::U32(100)),
                ("vote_end", ScVal::U32(200)),
                ("description", string("pay the auditor")),
            ]),
        )
    }

    #[test]
    fn matches_only_known_events_with_exact_arity() {
        let adapter = OpenZeppelinAdapter;
        let (topics, value) = created_event();
        let event = RawEvent {
            contract_id: GOVERNOR_CONTRACT_ID,
            ledger: 5_083_700,
            tx_hash: "00",
            topics: &topics,
            value: &value,
        };
        assert!(adapter.matches_event(&event));

        let short = &topics[..2];
        let truncated = RawEvent {
            topics: short,
            ..event
        };
        assert!(!adapter.matches_event(&truncated));

        let elsewhere = RawEvent {
            contract_id: "CDJWPKSQ4NA67PKTNJEPI6R2Q3JEDXPX5EDPM3YOSEHBDGBZ5THBTOKE",
            ..event
        };
        assert!(!adapter.matches_event(&elsewhere));
    }

    #[test]
    fn proposal_created_normalizes_to_verified_payload() {
        let adapter = OpenZeppelinAdapter;
        let (topics, value) = created_event();
        let event = RawEvent {
            contract_id: GOVERNOR_CONTRACT_ID,
            ledger: 5_083_700,
            tx_hash: "abc",
            topics: &topics,
            value: &value,
        };
        let normalized = adapter.normalize_event(&event).expect("valid event");
        assert_eq!(normalized.governor, ADAPTER_NAME);
        assert_eq!(normalized.decoding, DecodingStatus::Verified);
        assert_eq!(normalized.payload["vote_snapshot"], 100);
        assert_eq!(normalized.payload["vote_end"], 200);
        assert_eq!(normalized.payload["functions"][0], "transfer");
        assert_eq!(
            normalized.payload["proposal_id"].as_str().expect("id"),
            "11".repeat(32)
        );
    }

    #[test]
    fn created_proposal_decodes_each_call_and_applies_risk() {
        let adapter = OpenZeppelinAdapter;
        let (topics, value) = created_event();
        let event = RawEvent {
            contract_id: GOVERNOR_CONTRACT_ID,
            ledger: 5_083_700,
            tx_hash: "abc",
            topics: &topics,
            value: &value,
        };
        let normalized = adapter.normalize_event(&event).expect("valid event");
        let context = RiskContext::default();
        let proposal = adapter
            .proposal_from_created(&normalized.payload, &context)
            .expect("valid proposal");
        assert_eq!(proposal.calls.len(), 1);
        assert_eq!(proposal.calls[0].function_name, "transfer");
        assert_eq!(proposal.calls[0].decoding, DecodingStatus::Unverified);
        // No contextual rule fires for a plain transfer with default settings,
        // and the call's risk is explicitly not marked verified.
        assert_eq!(proposal.calls[0].risk.tier, crate::risk::RiskTier::Low);
        assert!(!proposal.calls[0].risk.verified);
        assert!(proposal.proposer.is_some());
    }

    #[test]
    fn vote_cast_accepts_the_oz_topic_order_only() {
        let adapter = OpenZeppelinAdapter;
        let topics = vec![
            sym("vote_cast"),
            contract_address(0xaa),
            proposal_id_bytes(0x22),
        ];
        let value = map(vec![
            ("vote_type", ScVal::U32(1)),
            ("weight", u128_val(u128::MAX)),
            ("reason", string("for")),
        ]);
        let event = RawEvent {
            contract_id: GOVERNOR_CONTRACT_ID,
            ledger: 1,
            tx_hash: "t",
            topics: &topics,
            value: &value,
        };
        let normalized = adapter.normalize_event(&event).expect("valid vote");
        assert_eq!(normalized.payload["vote_type"], 1);
        assert_eq!(normalized.payload["weight"], u128::MAX.to_string());

        // Script3 puts proposal_id before the voter; that must not match.
        let wrong_order = vec![
            sym("vote_cast"),
            proposal_id_bytes(0x22),
            contract_address(0xaa),
        ];
        let event = RawEvent {
            topics: &wrong_order,
            ..event
        };
        assert!(adapter.normalize_event(&event).is_err());
    }

    #[test]
    fn invalid_vote_type_and_missing_fields_fail_closed() {
        let adapter = OpenZeppelinAdapter;
        let topics = vec![
            sym("vote_cast"),
            contract_address(0xaa),
            proposal_id_bytes(0x22),
        ];
        let bad_type = map(vec![
            ("vote_type", ScVal::U32(3)),
            ("weight", u128_val(1)),
            ("reason", string("")),
        ]);
        let event = RawEvent {
            contract_id: GOVERNOR_CONTRACT_ID,
            ledger: 1,
            tx_hash: "t",
            topics: &topics,
            value: &bad_type,
        };
        assert!(adapter.normalize_event(&event).is_err());

        let missing_reason = map(vec![("vote_type", ScVal::U32(1)), ("weight", u128_val(1))]);
        let event = RawEvent {
            value: &missing_reason,
            ..event
        };
        assert!(adapter.normalize_event(&event).is_err());
    }

    #[test]
    fn terminal_and_quorum_events_decode() {
        let adapter = OpenZeppelinAdapter;
        let executed = vec![sym("proposal_executed"), proposal_id_bytes(0x22)];
        let event = RawEvent {
            contract_id: GOVERNOR_CONTRACT_ID,
            ledger: 1,
            tx_hash: "t",
            topics: &executed,
            value: &map(Vec::new()),
        };
        let normalized = adapter.normalize_event(&event).expect("executed");
        assert_eq!(
            normalized.payload["proposal_id"]
                .as_str()
                .expect("id")
                .len(),
            64
        );

        let quorum = vec![sym("quorum_changed")];
        let event = RawEvent {
            topics: &quorum,
            value: &map(vec![
                ("old_quorum", u128_val(1)),
                ("new_quorum", u128_val(2)),
            ]),
            ..event
        };
        let normalized = adapter.normalize_event(&event).expect("quorum");
        assert_eq!(normalized.payload["old_quorum"], "1");
        assert_eq!(normalized.payload["new_quorum"], "2");
    }

    #[test]
    fn proposal_state_maps_every_known_discriminant_and_rejects_unknown() {
        let adapter = OpenZeppelinAdapter;
        let id = "ab".repeat(32);
        for raw in 0..=7u32 {
            let state = adapter
                .state_from_chain(&id, &ScVal::U32(raw))
                .expect("known state");
            assert_eq!(state.status as u32, raw);
        }
        assert!(adapter.state_from_chain(&id, &ScVal::U32(8)).is_err());
        assert!(adapter.state_from_chain(&id, &ScVal::I32(1)).is_err());
        assert!(adapter
            .state_from_chain("not-a-proposal-id", &ScVal::U32(1))
            .is_err());
    }

    #[test]
    fn proposal_from_created_rejects_inconsistent_vectors() {
        let adapter = OpenZeppelinAdapter;
        let payload = serde_json::json!({
            "proposal_id": "ab".repeat(32),
            "proposer": "GAAA",
            "targets": ["CAAA"],
            "functions": [],
            "args": [[]],
        });
        assert!(adapter
            .proposal_from_created(&payload, &RiskContext::default())
            .is_err());
    }
}
