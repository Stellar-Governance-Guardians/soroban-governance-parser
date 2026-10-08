#![no_main]
// Fuzz the ScVal decoder on arbitrary bytes: decode XDR, then run the total
// ScVal -> JSON converter. Neither may panic; both must return Err on inputs
// they cannot represent.
use libfuzzer_sys::fuzz_target;
use stellar_xdr::{Limits, ReadXdr, ScVal};

fuzz_target!(|data: &[u8]| {
    if let Ok(scval) = ScVal::from_xdr(data, Limits::none()) {
        let _ = soroban_governance_core::scval::scval_to_json(&scval);
    }
});
