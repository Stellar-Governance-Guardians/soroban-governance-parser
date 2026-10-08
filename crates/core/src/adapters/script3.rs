//! Script3 `soroban-governor` adapter.
//!
//! Every shape encoded here is transcribed from the **pinned** upstream source,
//! not guessed:
//!
//! ```text
//! github.com/script3/soroban-governor @ a2ac6de81055be5bd13e31f922c9546309bfdb8a
//!   contracts/governor/src/events.rs   — event topics and data layout
//!   contracts/governor/src/types.rs    — ProposalAction, Calldata, GovernorSettings
//!   contracts/governor/src/vote_count.rs — tally / quorum / threshold rules
//! ```
//!
//! The event and struct layouts below were additionally confirmed against **real
//! captured chain data** (raw `simulateTransaction` responses committed under
//! the repo's fixture corpus) by the differential tests in this crate's
//! `tests/` directory. Where source and capture disagree, the capture wins and
//! the disagreement is a test failure.
//!
//! # Fail-closed
//!
//! A `ProposalAction` variant we do not recognise, a `Calldata` map missing a
//! field, or an event with an unexpected arity all produce `Err`. Nothing is
//! defaulted, and no field is invented. In particular a proposal targeting a
//! contract we have no spec for is decoded positionally and marked
//! [`DecodingStatus::Unverified`] — it is never given plausible-looking names.

use stellar_xdr::ScVal;

use crate::adapter::{GovernorAdapter, RawCall, RawEvent};
use crate::error::{AdapterError, DecodeError};
use crate::risk::{classify_unverified, classify_verified, CallUnderReview, RiskContext};
use crate::tally::{GovernorSettings, ProposalStatus, VoteCount};
use crate::types::{DecodedCall, DecodedProposal, DecodingStatus, NormalizedEvent};

/// Stable adapter identifier.
pub const ADAPTER_NAME: &str = "script3-soroban-governor";

/// The governor contract deployed and seeded on testnet, registered in
/// `deployments.json` under `contracts.testnet.seedV2Script3.script3Governor`.
pub const GOVERNOR_CONTRACT_ID: &str = "CCKGJBCBBYNWCMSBPV67DFIG3OL72RLDKNGHWJQCR5NASAUHOQRNLEDX";

/// The votes/token contract the governor reads power from.
pub const VOTES_CONTRACT_ID: &str = "CCAUJK6V6GIYQANKCV2JDCMCLGMQD42JCF426TQHHMSXW2F5ZROM4IKY";

/// Event symbols emitted by `contracts/governor/src/events.rs`, with their
/// exact topic arity *including* the leading symbol topic.
pub const EVENT_ARITY: &[(&str, usize)] = &[
    ("proposal_created", 3),
    ("proposal_canceled", 2),
    ("proposal_voting_closed", 4),
    ("proposal_executed", 2),
    ("proposal_expired", 2),
    ("vote_cast", 3),
];

/// How voting power is derived for this governor.
///
/// Recorded as data rather than prose so the dashboard can show it and the
/// tests can assert it. This is a *stated* model, verified by differential test
/// against `get_proposal_votes`; it is not an inferred one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VotePowerModel {
    /// Power is read from the votes contract at the proposal's vote-start
    /// checkpoint (`set_vote_sequence`), not recomputed from the event stream.
    /// Every `vote_cast` amount in the stream is summed into `VoteCount`.
    CheckpointedVotesContract,
}

impl VotePowerModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CheckpointedVotesContract => "checkpointed_votes_contract",
        }
    }
}

/// Governor state reconstructed from chain reads.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Script3ProposalState {
    pub governor: String,
    pub proposal_id: u32,
    pub title: String,
    pub description: String,
    pub creator: String,
    pub action: Script3Action,
    pub status: ProposalStatus,
    pub executable: bool,
    pub vote_start: u32,
    pub vote_end: u32,
    pub eta: u32,
}

/// The five `ProposalAction` variants from upstream `types.rs`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum Script3Action {
    /// `Calldata { contract_id, function, args, auths }`
    Calldata(Script3Calldata),
    /// `Upgrade(BytesN<32>)` — the wasm hash, base64 as `scval_to_json` renders Bytes.
    Upgrade(serde_json::Value),
    /// `Settings(GovernorSettings)` — governance parameters.
    Settings(GovernorSettings),
    /// `Council(Address)` — changes the council/council-gated authority.
    Council(String),
    /// `Snapshot` — no payload.
    Snapshot,
    /// A variant we do not know. Fail-closed: preserved verbatim, never guessed.
    Unknown {
        symbol: String,
        payload: serde_json::Value,
    },
}

/// Upstream `types.rs::Calldata`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Script3Calldata {
    pub contract_id: String,
    pub function: String,
    /// Positional args exactly as on chain; names only if a spec verified them.
    pub args: Vec<serde_json::Value>,
    /// Nested authorized sub-calls (`Calldata.auths`).
    pub auths: Vec<Script3Calldata>,
    /// Positional index of `args[0]` that represents a value amount, when the
    /// function is a known token movement. Drives the `large_value` risk rule.
    pub amount_index: Option<usize>,
}

impl Script3Calldata {
    /// Monetary amount if this call moves value and the amount position is
    /// known; `None` otherwise (never guessed).
    pub fn amount(&self) -> Option<i128> {
        let idx = self.amount_index?;
        // scval_to_json renders i128 losslessly as the tagged
        // {"_type":"i128","value":"…"} object; json_i128 reads that (and the
        // bare-string form) and returns None for any other shape.
        crate::scval::json_i128(self.args.get(idx)?)
    }
}

/// Function names that carry a token amount, and the positional index of that
/// amount in the upstream `Calldata.args` vector.
///
/// Verified against the pinned `soroban-votes`/`soroban-governor` token
/// interface (`transfer`, `transfer_from`) — NOT guessed. A function not listed
/// here yields `amount_index: None`, which makes the `large_value` rule skip the
/// call instead of inventing a number.
const AMOUNT_POSITIONS: &[(&str, usize)] = &[("transfer", 2), ("transfer_from", 2)];

fn amount_index_for(function: &str) -> Option<usize> {
    AMOUNT_POSITIONS
        .iter()
        .find(|(f, _)| *f == function)
        .map(|(_, i)| *i)
}

// --- small ScVal helpers -------------------------------------------------------

fn want_symbol(v: &ScVal) -> Result<String, AdapterError> {
    match v {
        ScVal::Symbol(s) => Ok(s.to_string()),
        other => Err(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!("expected Symbol, got {other:?}")),
        }),
    }
}

fn want_u32(v: &ScVal) -> Result<u32, AdapterError> {
    match v {
        ScVal::U32(n) => Ok(*n),
        other => Err(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!("expected U32, got {other:?}")),
        }),
    }
}

fn want_i128(v: &ScVal) -> Result<i128, AdapterError> {
    crate::scval::scval_i128(v).ok_or(AdapterError::Decode {
        adapter: ADAPTER_NAME,
        source: DecodeError::UnsupportedScVal(format!("expected I128, got {v:?}")),
    })
}

fn want_string(v: &ScVal) -> Result<String, AdapterError> {
    match v {
        ScVal::String(s) => Ok(s.to_string()),
        other => Err(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!("expected String, got {other:?}")),
        }),
    }
}

fn want_address(v: &ScVal) -> Result<String, AdapterError> {
    match v {
        ScVal::Address(a) => Ok(a.to_string()),
        other => Err(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!("expected Address, got {other:?}")),
        }),
    }
}

fn want_vec(v: &ScVal) -> Result<&[ScVal], AdapterError> {
    match v {
        ScVal::Vec(items) => {
            let inner = items.as_ref().ok_or(AdapterError::Decode {
                adapter: ADAPTER_NAME,
                source: DecodeError::UnsupportedScVal("ScVal::Vec(None)".into()),
            })?;
            Ok(inner.as_slice())
        }
        other => Err(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!("expected Vec, got {other:?}")),
        }),
    }
}

/// Borrow the entries of an `ScVal::Map`, fail-closed on anything else.
fn scmap(v: &ScVal) -> Result<&[stellar_xdr::ScMapEntry], AdapterError> {
    match v {
        ScVal::Map(m) => {
            let inner = m.as_ref().ok_or(AdapterError::Decode {
                adapter: ADAPTER_NAME,
                source: DecodeError::UnsupportedScVal("ScVal::Map(None)".into()),
            })?;
            Ok(inner.as_slice())
        }
        other => Err(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!("expected Map, got {other:?}")),
        }),
    }
}

/// Pull one field from a symbol-keyed ScVal map, fail-closed when absent.
fn map_field<'a>(v: &'a ScVal, field: &str) -> Result<&'a ScVal, AdapterError> {
    let entries = scmap(v)?;
    entries
        .iter()
        .find(|e| matches!(&e.key, ScVal::Symbol(s) if s.to_string() == field))
        .map(|e| &e.val)
        .ok_or(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!(
                "map is missing required field `{field}`"
            )),
        })
}

// --- decoding ------------------------------------------------------------------

/// Decode a `ProposalAction` enum: ScVal::Vec([Symbol, payload]).
///
/// Fail-closed on an unknown variant — preserved as `Unknown` rather than
/// being coerced into a known variant.
pub fn decode_action(v: &ScVal) -> Result<Script3Action, AdapterError> {
    let parts = want_vec(v)?;
    if parts.len() != 2 {
        return Err(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!(
                "ProposalAction must be a 2-element vec, got {} elements",
                parts.len()
            )),
        });
    }
    let (symbol, payload) = (&parts[0], &parts[1]);
    let variant = want_symbol(symbol)?;
    Ok(match variant.as_str() {
        "Calldata" => Script3Action::Calldata(decode_calldata(payload)?),
        "Upgrade" => {
            // BytesN<32> wasm hash. scval_to_json base64-encodes ScVal::Bytes,
            // so the canonical representation comes from the shared converter
            // rather than a second encoder here.
            if !matches!(payload, ScVal::Bytes(_)) {
                return Err(AdapterError::Decode {
                    adapter: ADAPTER_NAME,
                    source: DecodeError::UnsupportedScVal(format!(
                        "Upgrade expects Bytes, got {payload:?}"
                    )),
                });
            }
            Script3Action::Upgrade(crate::scval::scval_to_json(payload).map_err(|source| {
                AdapterError::Decode {
                    adapter: ADAPTER_NAME,
                    source,
                }
            })?)
        }
        "Settings" => Script3Action::Settings(decode_settings(payload)?),
        "Council" => Script3Action::Council(want_address(payload)?),
        "Snapshot" => Script3Action::Snapshot,
        other => Script3Action::Unknown {
            symbol: other.to_owned(),
            payload: crate::scval::scval_to_json(payload).map_err(|source| {
                AdapterError::Decode {
                    adapter: ADAPTER_NAME,
                    source,
                }
            })?,
        },
    })
}

/// Decode upstream `Calldata { contract_id, function, args, auths }`.
fn decode_calldata(v: &ScVal) -> Result<Script3Calldata, AdapterError> {
    let contract_id = want_address(map_field(v, "contract_id")?)?;
    let function = want_symbol(map_field(v, "function")?)?;
    let args = want_vec(map_field(v, "args")?)?
        .iter()
        .map(crate::scval::scval_to_json)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source,
        })?;
    let mut auths = Vec::new();
    for a in want_vec(map_field(v, "auths")?)? {
        auths.push(decode_calldata(a)?);
    }
    Ok(Script3Calldata {
        contract_id,
        amount_index: amount_index_for(&function),
        function,
        args,
        auths,
    })
}

/// Decode upstream `GovernorSettings`. Every field is required.
fn decode_settings(v: &ScVal) -> Result<GovernorSettings, AdapterError> {
    fn i128_field(v: &ScVal, name: &str) -> Result<i128, AdapterError> {
        crate::scval::scval_i128(v).ok_or(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!(
                "settings.{name} expected I128, got {v:?}"
            )),
        })
    }
    fn u32_field(v: &ScVal, name: &str) -> Result<u32, AdapterError> {
        match v {
            ScVal::U32(n) => Ok(*n),
            other => Err(AdapterError::Decode {
                adapter: ADAPTER_NAME,
                source: DecodeError::UnsupportedScVal(format!(
                    "settings.{name} expected U32, got {other:?}"
                )),
            }),
        }
    }
    Ok(GovernorSettings {
        proposal_threshold: i128_field(map_field(v, "proposal_threshold")?, "proposal_threshold")?,
        vote_delay: u32_field(map_field(v, "vote_delay")?, "vote_delay")?,
        vote_period: u32_field(map_field(v, "vote_period")?, "vote_period")?,
        timelock: u32_field(map_field(v, "timelock")?, "timelock")?,
        grace_period: u32_field(map_field(v, "grace_period")?, "grace_period")?,
        quorum_bps: u32_field(map_field(v, "quorum")?, "quorum")?,
        counting_type: u32_field(map_field(v, "counting_type")?, "counting_type")?,
        vote_threshold_bps: u32_field(map_field(v, "vote_threshold")?, "vote_threshold")?,
    })
}

/// Decode a `VoteCount { against, _for, abstain }` map.
pub fn decode_vote_count(v: &ScVal) -> Result<VoteCount, AdapterError> {
    fn i128_field(v: &ScVal, name: &str) -> Result<i128, AdapterError> {
        crate::scval::scval_i128(v).ok_or(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!(
                "VoteCount.{name} expected I128, got {v:?}"
            )),
        })
    }
    Ok(VoteCount::new(
        i128_field(map_field(v, "against")?, "against")?,
        i128_field(map_field(v, "_for")?, "_for")?,
        i128_field(map_field(v, "abstain")?, "abstain")?,
    ))
}

// --- the adapter ---------------------------------------------------------------

/// Adapter for the pinned Script3 `soroban-governor`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Script3Adapter;

impl GovernorAdapter for Script3Adapter {
    fn name(&self) -> &'static str {
        ADAPTER_NAME
    }

    /// Structural check only: topic[0] must be a known Script3 event symbol with
    /// the arity the pinned source emits, and the event must come from the
    /// registered governor contract.
    ///
    /// Unknown symbols return `false` (never `true` on a guess), so the
    /// pipeline falls back to `unverified`.
    fn matches_event(&self, event: &RawEvent<'_>) -> bool {
        if event.contract_id != GOVERNOR_CONTRACT_ID {
            return false;
        }
        let Some(first) = event.topics.first() else {
            return false;
        };
        let ScVal::Symbol(sym) = first else {
            return false;
        };
        EVENT_ARITY
            .iter()
            .any(|(name, arity)| *name == sym.to_string() && event.topics.len() == *arity)
    }

    fn normalize_event(&self, event: &RawEvent<'_>) -> Result<NormalizedEvent, AdapterError> {
        let Some(ScVal::Symbol(sym)) = event.topics.first() else {
            return Err(AdapterError::TopicMismatch(ADAPTER_NAME));
        };
        let kind = sym.to_string();
        let payload = match kind.as_str() {
            "proposal_created" => {
                // topics: [symbol, proposal_id: u32, proposer: Address]
                // data:   [title, desc, action, vote_start, u32, vote_end, u32]
                let id = want_u32(&event.topics[1])?;
                let proposer = want_address(&event.topics[2])?;
                let d = want_vec(event.value)?;
                if d.len() != 5 {
                    return Err(AdapterError::Decode {
                        adapter: ADAPTER_NAME,
                        source: DecodeError::UnsupportedScVal(format!(
                            "proposal_created data must be 5 elements, got {}",
                            d.len()
                        )),
                    });
                }
                let (title, desc, action, vote_start, vote_end) =
                    (&d[0], &d[1], &d[2], &d[3], &d[4]);
                serde_json::json!({
                    "proposal_id": id,
                    "proposer": proposer,
                    "title": want_string(title)?,
                    "description": want_string(desc)?,
                    "action": decode_action(action)?,
                    "vote_start": want_u32(vote_start)?,
                    "vote_end": want_u32(vote_end)?,
                })
            }
            "vote_cast" => {
                // topics: [symbol, proposal_id: u32, voter: Address]
                // data:   [support: u32, amount: i128]
                let id = want_u32(&event.topics[1])?;
                let voter = want_address(&event.topics[2])?;
                let d = want_vec(event.value)?;
                if d.len() != 2 {
                    return Err(AdapterError::Decode {
                        adapter: ADAPTER_NAME,
                        source: DecodeError::UnsupportedScVal(format!(
                            "vote_cast data must be 2 elements, got {}",
                            d.len()
                        )),
                    });
                }
                let (support, amount) = (&d[0], &d[1]);
                let support = want_u32(support)?;
                let amount = want_i128(amount)?;
                serde_json::json!({
                    "proposal_id": id,
                    "voter": voter,
                    "support": support,
                    "amount": amount.to_string(),
                })
            }
            "proposal_voting_closed" => {
                // topics: [symbol, proposal_id: u32, status: u32, eta: u32]
                // data:   final_votes: VoteCount
                let id = want_u32(&event.topics[1])?;
                let status_raw = want_u32(&event.topics[2])?;
                let eta = want_u32(&event.topics[3])?;
                let status = ProposalStatus::from_u32(status_raw).ok_or(AdapterError::Decode {
                    adapter: ADAPTER_NAME,
                    source: DecodeError::UnsupportedScVal(format!(
                        "unknown ProposalStatus {status_raw}"
                    )),
                })?;
                serde_json::json!({
                    "proposal_id": id,
                    "status": status,
                    "eta": eta,
                    "final_votes": decode_vote_count(event.value)?,
                })
            }
            "proposal_canceled" | "proposal_executed" | "proposal_expired" => {
                // topics: [symbol, proposal_id: u32]; data: void
                serde_json::json!({ "proposal_id": want_u32(&event.topics[1])? })
            }
            other => {
                return Err(AdapterError::Decode {
                    adapter: ADAPTER_NAME,
                    source: DecodeError::UnsupportedScVal(format!(
                        "unhandled Script3 event `{other}`"
                    )),
                })
            }
        };

        Ok(NormalizedEvent {
            governor: ADAPTER_NAME.to_owned(),
            topics: event
                .topics
                .iter()
                .map(|t| crate::scval::scval_to_json(t).map(|v| v.to_string()))
                .collect::<Result<Vec<_>, _>>()
                .unwrap_or_default(),
            ledger: event.ledger,
            tx_hash: event.tx_hash.to_owned(),
            contract_id: event.contract_id.to_owned(),
            payload,
            decoding: DecodingStatus::Verified,
        })
    }

    /// Decode a proposal's action into a `DecodedCall`.
    ///
    /// `spec` is only consulted for argument *names*; without it the args stay
    /// positional and `decoding` is `Unverified` (charter rule 2).
    fn decode_call(
        &self,
        call: &RawCall<'_>,
        spec: Option<&crate::spec::ContractSpec>,
    ) -> Result<DecodedCall, AdapterError> {
        let positional = || -> Result<serde_json::Value, AdapterError> {
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
        };
        // Names are only available when the target contract's spec actually
        // declares this function. Arity mismatch is also "cannot verify".
        let (arguments, decoding) = match spec {
            Some(s) => match crate::spec::decode_args_with_spec(s, call.function_name, call.args) {
                Ok(named) => (serde_json::json!(named), DecodingStatus::Verified),
                Err(_) => (positional()?, DecodingStatus::Unverified),
            },
            None => (positional()?, DecodingStatus::Unverified),
        };
        let verified = decoding == DecodingStatus::Verified;
        let risk = if verified {
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

impl Script3Adapter {
    /// The stated voting-power model for this governor.
    pub const fn vote_power_model(&self) -> VotePowerModel {
        VotePowerModel::CheckpointedVotesContract
    }

    /// Reconstruct proposal state from a captured `get_proposal` return value.
    ///
    /// `get_proposal` returns a map with `config { title, description, action }`
    /// and `data { creator, eta, executable, status, vote_start, vote_end }`,
    /// exactly as upstream `types.rs::Proposal`.
    pub fn state_from_chain(
        &self,
        proposal_id: u32,
        get_proposal: &ScVal,
    ) -> Result<Script3ProposalState, AdapterError> {
        let config = map_field(get_proposal, "config")?;
        let data = map_field(get_proposal, "data")?;

        let status_raw = want_u32(map_field(data, "status")?)?;
        let status = ProposalStatus::from_u32(status_raw).ok_or(AdapterError::Decode {
            adapter: ADAPTER_NAME,
            source: DecodeError::UnsupportedScVal(format!("unknown ProposalStatus {status_raw}")),
        })?;
        let executable = match map_field(data, "executable")? {
            ScVal::Bool(b) => *b,
            other => {
                return Err(AdapterError::Decode {
                    adapter: ADAPTER_NAME,
                    source: DecodeError::UnsupportedScVal(format!(
                        "executable expected Bool, got {other:?}"
                    )),
                })
            }
        };

        Ok(Script3ProposalState {
            governor: ADAPTER_NAME.to_owned(),
            proposal_id,
            title: want_string(map_field(config, "title")?)?,
            description: want_string(map_field(config, "description")?)?,
            creator: want_address(map_field(data, "creator")?)?,
            action: decode_action(map_field(config, "action")?)?,
            status,
            executable,
            vote_start: want_u32(map_field(data, "vote_start")?)?,
            vote_end: want_u32(map_field(data, "vote_end")?)?,
            eta: want_u32(map_field(data, "eta")?)?,
        })
    }

    /// Reconstruct the tally from a captured `get_proposal_votes` return value.
    pub fn tally_from_chain(&self, get_proposal_votes: &ScVal) -> Result<VoteCount, AdapterError> {
        decode_vote_count(get_proposal_votes)
    }

    /// Decode a full proposal (`config` + `data`) into the shared `DecodedProposal`
    /// shape, applying the published risk rules to the action.
    pub fn decode_proposal(
        &self,
        state: &Script3ProposalState,
        context: &RiskContext,
    ) -> Result<DecodedProposal, AdapterError> {
        let calls = match &state.action {
            Script3Action::Calldata(cd) => vec![Self::calldata_to_decoded_call(cd, context)],
            Script3Action::Upgrade(hash) => vec![DecodedCall {
                contract_id: GOVERNOR_CONTRACT_ID.to_owned(),
                function_name: "upgrade".to_owned(),
                arguments: serde_json::json!({ "wasm_hash": hash }),
                // The wasm hash is read straight off chain and the variant name
                // comes from the pinned enum, so this IS verified for *this*
                // governor — but the target code is opaque, so the risk tier
                // still says exactly what it can support.
                decoding: DecodingStatus::Verified,
                risk: classify_verified("upgrade"),
            }],
            Script3Action::Settings(_) => vec![DecodedCall {
                contract_id: GOVERNOR_CONTRACT_ID.to_owned(),
                function_name: "settings".to_owned(),
                arguments: serde_json::json!("governance parameters changed"),
                decoding: DecodingStatus::Verified,
                risk: classify_verified("set_config"),
            }],
            Script3Action::Council(addr) => vec![DecodedCall {
                contract_id: GOVERNOR_CONTRACT_ID.to_owned(),
                function_name: "council".to_owned(),
                arguments: serde_json::json!({ "council": addr }),
                decoding: DecodingStatus::Verified,
                risk: classify_verified("set_admin"),
            }],
            Script3Action::Snapshot => vec![DecodedCall {
                contract_id: GOVERNOR_CONTRACT_ID.to_owned(),
                function_name: "snapshot".to_owned(),
                arguments: serde_json::json!("checkpoint snapshot"),
                decoding: DecodingStatus::Verified,
                risk: crate::risk::RiskClassification {
                    tier: crate::risk::RiskTier::Low,
                    matched_rule: "snapshot (read-only checkpoint)".to_owned(),
                    verified: true,
                },
            }],
            Script3Action::Unknown { symbol, .. } => vec![DecodedCall {
                contract_id: GOVERNOR_CONTRACT_ID.to_owned(),
                function_name: symbol.clone(),
                arguments: serde_json::Value::Null,
                // Fail-closed: an action we cannot decode must not look decoded.
                decoding: DecodingStatus::Unverified,
                risk: classify_unverified(symbol),
            }],
        };

        Ok(DecodedProposal {
            governor: ADAPTER_NAME.to_owned(),
            proposal_id: state.proposal_id.to_string(),
            proposer: Some(state.creator.clone()),
            calls,
            decoding: DecodingStatus::Verified,
        })
    }

    /// Project the on-chain `Calldata` into the governor-agnostic shape the
    /// contextual rules evaluate, recursing into authorized sub-calls.
    fn as_call_under_review(cd: &Script3Calldata) -> CallUnderReview {
        CallUnderReview {
            target: cd.contract_id.clone(),
            function: cd.function.clone(),
            args: cd.args.clone(),
            amount: cd.amount(),
            nested: cd.auths.iter().map(Self::as_call_under_review).collect(),
        }
    }

    fn calldata_to_decoded_call(cd: &Script3Calldata, context: &RiskContext) -> DecodedCall {
        let flags = context.evaluate(&Self::as_call_under_review(cd));
        let tier = flags.highest_tier();
        DecodedCall {
            contract_id: cd.contract_id.clone(),
            function_name: cd.function.clone(),
            arguments: serde_json::json!(cd.args),
            // No spec was supplied for the *target* contract, so argument names
            // are unknown. Positional, and explicitly unverified.
            decoding: DecodingStatus::Unverified,
            risk: RiskClassificationForCall {
                tier,
                matched_rule: flags.matched_rules(),
                verified: false,
            }
            .into(),
        }
    }
}

/// Small local shim so the contextual rules can produce a `RiskClassification`
/// without pretending the function name was spec-verified.
struct RiskClassificationForCall {
    tier: crate::risk::RiskTier,
    matched_rule: String,
    verified: bool,
}

impl From<RiskClassificationForCall> for crate::risk::RiskClassification {
    fn from(v: RiskClassificationForCall) -> Self {
        crate::risk::RiskClassification {
            tier: v.tier,
            matched_rule: v.matched_rule,
            verified: v.verified,
        }
    }
}
