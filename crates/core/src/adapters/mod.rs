//! Concrete governor adapters. Each module owns everything it knows about one
//! governor; the core pipeline only ever talks to the `GovernorAdapter` trait.

pub mod openzeppelin;
pub mod script3;

use crate::adapter::GovernorAdapter;
use openzeppelin::OpenZeppelinAdapter;
use script3::Script3Adapter;

static SCRIPT3: Script3Adapter = Script3Adapter;
static OPENZEPPELIN: OpenZeppelinAdapter = OpenZeppelinAdapter;

/// Every adapter this build knows, in selection order. Adapters are matched
/// structurally against the event, so order only breaks ties between adapters
/// that would both claim the same event — which the tests forbid.
pub fn registry() -> [&'static dyn GovernorAdapter; 2] {
    [&SCRIPT3, &OPENZEPPELIN]
}

/// Adapter id for a known governor contract, or `None`. Used by the IO layer to
/// report which governor an event belongs to without decoding it.
pub fn governor_for_contract(contract_id: &str) -> Option<&'static str> {
    match contract_id {
        script3::GOVERNOR_CONTRACT_ID => Some(script3::ADAPTER_NAME),
        openzeppelin::GOVERNOR_CONTRACT_ID => Some(openzeppelin::ADAPTER_NAME),
        _ => None,
    }
}
