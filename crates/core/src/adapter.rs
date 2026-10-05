//! Governor-agnostic adapter trait (charter rule 3).
//!
//! All governor-specific knowledge lives behind `GovernorAdapter`. The core
//! pipeline never branches on a governor name outside an adapter. Adapters
//! are sans-IO: they receive already-fetched data and return normalized
//! results or explicit errors.

use stellar_xdr::ScVal;

use crate::error::AdapterError;
use crate::types::{DecodedCall, DecodingStatus, NormalizedEvent};

/// Raw event data as captured from the chain (by an IO layer), handed to an
/// adapter for normalization. Base64 XDR strings are decoded by the caller
/// into ScVal trees so adapters stay pure.
pub struct RawEvent<'a> {
    pub contract_id: &'a str,
    pub ledger: u32,
    pub tx_hash: &'a str,
    pub topics: &'a [ScVal],
    pub value: &'a ScVal,
}

/// Raw invocation data from a transaction envelope or proposal calldata.
pub struct RawCall<'a> {
    pub contract_id: &'a str,
    pub function_name: &'a str,
    pub args: &'a [ScVal],
}

pub trait GovernorAdapter {
    /// Stable adapter identifier, e.g. "script3-soroban-governor".
    fn name(&self) -> &'static str;

    /// Cheap structural check: does this event look like it belongs to this
    /// governor? Must be based on verified topic shapes only (see
    /// docs/adapters/<name>.md). Returns false rather than guessing.
    fn matches_event(&self, event: &RawEvent<'_>) -> bool;

    /// Normalize a matched event. Fail-closed: if the payload does not match
    /// the verified shape, return Err — never a partial or defaulted decode.
    fn normalize_event(&self, event: &RawEvent<'_>) -> Result<NormalizedEvent, AdapterError>;

    /// Decode a proposal's contract call into a `DecodedCall`. When the
    /// contract spec is unavailable, implementations MUST return the raw
    /// positional args with `decoding: Unverified` — never invented names.
    fn decode_call(
        &self,
        call: &RawCall<'_>,
        spec: Option<&crate::spec::ContractSpec>,
    ) -> Result<DecodedCall, AdapterError>;
}

/// Selection helper: first adapter whose `matches_event` returns true wins.
/// If none matches, the result is explicitly `Unverified` — the pipeline
/// never fabricates a governor attribution.
pub fn select_adapter<'a>(
    adapters: &'a [&'a dyn GovernorAdapter],
    event: &RawEvent<'_>,
) -> Option<&'a dyn GovernorAdapter> {
    adapters.iter().copied().find(|a| a.matches_event(event))
}

/// Fallback used when no adapter matches: emits the raw ScVal tree with
/// `decoding: unverified`, per charter rule 2/3.
pub fn unverified_event(event: &RawEvent<'_>) -> Result<NormalizedEvent, AdapterError> {
    let topics = event
        .topics
        .iter()
        .map(|t| match crate::scval::scval_to_json(t) {
            Ok(v) => Ok(v.to_string()),
            Err(e) => Err(AdapterError::Decode {
                adapter: "unverified",
                source: e,
            }),
        })
        .collect::<Result<Vec<String>, AdapterError>>()?;
    let payload = crate::scval::scval_to_json(event.value).map_err(|e| AdapterError::Decode {
        adapter: "unverified",
        source: e,
    })?;
    Ok(NormalizedEvent {
        governor: "unverified".into(),
        topics,
        ledger: event.ledger,
        tx_hash: event.tx_hash.to_owned(),
        contract_id: event.contract_id.to_owned(),
        payload,
        decoding: DecodingStatus::Unverified,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    struct NoMatch;
    impl GovernorAdapter for NoMatch {
        fn name(&self) -> &'static str {
            "no-match"
        }
        fn matches_event(&self, _event: &RawEvent<'_>) -> bool {
            false
        }
        fn normalize_event(&self, _event: &RawEvent<'_>) -> Result<NormalizedEvent, AdapterError> {
            Err(AdapterError::TopicMismatch("no-match"))
        }
        fn decode_call(
            &self,
            _call: &RawCall<'_>,
            _spec: Option<&crate::spec::ContractSpec>,
        ) -> Result<DecodedCall, AdapterError> {
            Err(AdapterError::UnknownGovernor("no-match".into()))
        }
    }

    fn sample_event() -> (ScVal, ScVal) {
        (
            ScVal::Symbol("fee".try_into().expect("short symbol")),
            ScVal::U32(1),
        )
    }

    #[test]
    fn unmatched_event_falls_back_to_unverified() {
        let (topic, value) = sample_event();
        let raw = RawEvent {
            contract_id: "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            ledger: 100,
            tx_hash: "deadbeef",
            topics: std::slice::from_ref(&topic),
            value: &value,
        };
        let adapters: [&dyn GovernorAdapter; 1] = [&NoMatch];
        assert!(select_adapter(&adapters, &raw).is_none());
        let out = unverified_event(&raw).expect("always succeeds for representable ScVals");
        assert_eq!(out.decoding, DecodingStatus::Unverified);
        assert_eq!(out.governor, "unverified");
    }
}
